// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import struct BsvHomeCore.BsvHomeConfirmInput
import struct BsvHomeCore.BsvHomeConfirmLoad
import class BsvHomeCore.BsvHomeNameService
import enum BsvHomeCore.BsvHomePaymailError
import enum BsvHomeCore.BsvHomeTransferAmountResult
import struct BsvHomeCore.BsvHomeTransferData
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Localization
import Primitives
import PrimitivesTestKit
import Testing
@testable import Transfer

@MainActor
struct WowPaySendViewModelTests {
    private let address = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA"
    private let tronAddress = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"

    @Test
    func walletRecipientsUseTheFixedSenderAndPreserveTheSelectedWalletName() async throws {
        var confirmed: BsvHomeTransferData?
        let model = makeModel(onTransferAction: { confirmed = $0 }, feeLoader: { transfer in
            validFeeLoad(fee: 500, transfer: transfer)
        })
        let destination = Wallet.mock(
            id: .mock(address: "destination-wallet"),
            name: "Receiving wallet",
            accounts: [.mock(chain: .bitcoinSv, address: "1BoatSLRHtKNngkdXEeobR76b53LETtpyT")],
        )
        await model.refreshWalletRecipients(wallets: [model.wallet, destination])

        #expect(model.walletRecipientsState.value != nil)
        let row = try #require(model.walletRecipients.first)
        #expect(model.walletRecipients.count == 1)
        #expect(row.walletId == destination.id.id)
        model.isPresentingContacts = true
        model.onSelectWallet(row)
        #expect(model.addressInputModel.text == destination.accounts[0].address)
        #expect(!model.isPresentingContacts)
        #expect(model.recipientError == nil)
        model.amountInputModel.text = "0.1"
        model.onChangeAmountText("", "0.1")
        try await waitFor { model.isConfirmEnabled }
        model.onSelectConfirm()
        #expect(confirmed?.recipient.name == destination.name)
    }

    @Test
    func malformedWalletRecipientShowsAnErrorAndCanBeRetried() async {
        let model = makeModel(autoPrepare: false)
        let damaged = Wallet.mock(
            id: .mock(address: "damaged-wallet"),
            accounts: [.mock(chain: .bitcoinSv, address: "damaged-bsv-address")],
        )
        await model.refreshWalletRecipients(wallets: [damaged])
        #expect(model.walletRecipientsState.isError)
        #expect(model.walletRecipients.isEmpty)

        await model.refreshWalletRecipients(wallets: [model.wallet])
        #expect(model.walletRecipientsState.value != nil)
        #expect(model.walletRecipients.isEmpty)
    }

    @Test(arguments: [false, true])
    func senderRequiredUsesTheExplicitManagementAction(hasManagementAction: Bool) async throws {
        var openedManagement = false
        let service = BsvHomeSendServiceMock { _, _, _, _ in throw BsvHomePaymailError.SenderRequired }
        let model = makeModel(service: service, onManagePaymail: hasManagementAction ? { openedManagement = true } : nil)
        setPaymailInputs(on: model, handle: "alice@example.com", amount: "0.1")
        try await waitFor { model.recipientError != nil }

        #expect(model.requiresPaymailSender)
        #expect(model.recipientError == Localized.wowpayPaymailSenderRequired)
        #expect(model.canManagePaymailSender == hasManagementAction)
        #expect(!model.isConfirmEnabled)
        model.onSelectManagePaymail()
        #expect(openedManagement == hasManagementAction)
    }

    @Test(arguments: ["WOWPAY-MSIG:1:obsolete", "{\"network\":\"BSV_MAINNET\",\"type\":\"P2MS\",\"policy\":{\"threshold\":2,\"publicKeys\":[]}}"])
    func publicConfigurationAndObsoletePackagesCannotStartRegularPayment(input: String) {
        var confirmed: BsvHomeTransferData?
        let model = makeModel(onTransferAction: { confirmed = $0 })
        model.addressInputModel.text = input
        model.onChangeAddressText("", new: input)
        model.amountInputModel.text = "0.0001"
        model.onChangeAmountText("", "0.0001")
        #expect(model.recipientError != nil)
        #expect(!model.isConfirmEnabled)
        model.onSelectConfirm()
        #expect(confirmed == nil)
    }

    @Test
    func unsupportedMultisigScanCannotRestorePreviousRecipientQuote() async throws {
        var confirmed: BsvHomeTransferData?
        let model = makeModel(onTransferAction: { confirmed = $0 }, feeLoader: { transfer in
            validFeeLoad(fee: 500, transfer: transfer)
        })
        setValidInputs(on: model, amount: "0.0001")
        try await waitFor { model.isConfirmEnabled }
        model.onHandleScan(#"{"network":"BSV_MAINNET","type":"P2MS","policy":{"threshold":2,"publicKeys":[]}}"#)
        try await waitFor { !model.isEstimatingFee }
        #expect(model.addressInputModel.text.isEmpty)
        #expect(model.recipientError != nil)
        #expect(!model.isConfirmEnabled)
        model.onSelectConfirm()
        #expect(confirmed == nil)
    }

    @Test
    func bsvAuthenticationFailureBlocksFeesUntilExplicitRetry() async throws {
        var attempts = 0
        var fees = 0
        let model = makeModel(autoPrepare: false, prepareAddresses: {
            attempts += 1
            if attempts == 1 {
                throw CancellationError()
            }
        }, feeLoader: { transfer in
            fees += 1
            return validFeeLoad(fee: 500, transfer: transfer)
        })
        setValidInputs(on: model, amount: "0.1")
        #expect(fees == 0)
        #expect(!model.isConfirmEnabled)

        await model.prepareBsvAddresses()
        #expect(model.addressesState.isError)
        #expect(fees == 0)
        #expect(!model.isConfirmEnabled)

        await model.prepareBsvAddresses()
        try await waitFor { model.isConfirmEnabled }
        #expect(attempts == 2)
        #expect(fees == 1)
    }

    @Test
    func paymailKeepsHandleAndResolvedAddressForConfirmation() async throws {
        var confirmed: BsvHomeTransferData?
        let service = BsvHomeSendServiceMock { asset, handle, value, useMaxAmount in
            .mock(type: .transfer(asset.map()), recipient: .mock(name: handle, address: address), value: value, useMaxAmount: useMaxAmount)
        }
        let model = makeModel(service: service, onTransferAction: { confirmed = $0 }, feeLoader: { transfer in
            validFeeLoad(fee: 500, transfer: transfer)
        })
        setPaymailInputs(on: model, handle: "alice@example.com", amount: "0.3")

        try await waitFor { model.isConfirmEnabled }
        #expect(service.inputs == ["alice@example.com"])
        #expect(model.paymailResolutionState == .resolved)
        model.onSelectConfirm()
        #expect(confirmed?.recipient.name == "alice@example.com")
        #expect(confirmed?.recipient.address == address)
        #expect(confirmed?.value == "30000000")
        confirmed = nil
        model.amountInputModel.text = "0.00001"
        #expect(!model.isConfirmEnabled)
        #expect(model.paymailResolutionState == .none)
        model.onSelectConfirm()
        #expect(confirmed == nil)
    }

    @Test(arguments: [false, true])
    func oldPaymailTransferCannotReplaceChangedInput(changeAmount: Bool) async throws {
        var firstResponse: CheckedContinuation<BsvHomeTransferData, any Error>?
        var confirmed: BsvHomeTransferData?
        var destinationCount = 0
        var feeCount = 0
        let service = BsvHomeSendServiceMock { asset, handle, value, useMaxAmount in
            destinationCount += 1
            if destinationCount == 1 {
                return try await withCheckedThrowingContinuation { firstResponse = $0 }
            }
            return .mock(type: .transfer(asset.map()), recipient: .mock(name: handle, address: address), value: value, useMaxAmount: useMaxAmount)
        }
        let model = makeModel(service: service, onTransferAction: { confirmed = $0 }, feeLoader: { transfer in
            feeCount += 1
            return validFeeLoad(fee: 500, transfer: transfer)
        })
        setPaymailInputs(on: model, handle: "alice@example.com", amount: "0.1")
        try await waitFor { firstResponse != nil }
        #expect(model.paymailResolutionState == .resolving)

        if changeAmount {
            model.amountInputModel.text = "0.2"
            model.onChangeAmountText("0.1", "0.2")
        } else {
            model.addressInputModel.text = "bob@example.com"
            model.onChangeAddressText("alice@example.com", new: "bob@example.com")
        }
        #expect(!model.isConfirmEnabled)
        try await waitFor { model.isConfirmEnabled }
        firstResponse?.resume(returning: .mock(type: .transfer(Chain.bitcoinSv.asset), recipient: .mock(name: "alice@example.com", address: "1BoatSLRHtKNngkdXEeobR76b53LETtpyT"), value: 10_000_000))
        try await Task.sleep(for: .milliseconds(20))
        #expect(destinationCount == 2)
        #expect(feeCount == 1)
        model.onSelectConfirm()
        #expect(confirmed?.recipient.name == (changeAmount ? "alice@example.com" : "bob@example.com"))
        #expect(confirmed?.recipient.address == address)
        #expect(confirmed?.value == (changeAmount ? "20000000" : "10000000"))
    }

    @Test
    func paymailFailureBlocksFeesAndCanBeRetried() async throws {
        var shouldFail = true
        var feeCount = 0
        let service = BsvHomeSendServiceMock { asset, handle, value, useMaxAmount in
            if shouldFail {
                throw BsvHomePaymailError.UnsupportedOutput
            }
            return .mock(type: .transfer(asset.map()), recipient: .mock(name: handle, address: address), value: value, useMaxAmount: useMaxAmount)
        }
        let model = makeModel(service: service, feeLoader: { transfer in
            feeCount += 1
            return validFeeLoad(fee: 500, transfer: transfer)
        })
        setPaymailInputs(on: model, handle: "alice@example.com", amount: "0.1")
        try await waitFor { model.recipientError != nil }
        #expect(model.recipientError == Localized.Errors.notSupported)
        #expect(model.paymailResolutionState == .none)
        #expect(!model.isConfirmEnabled)
        #expect(feeCount == 0)

        shouldFail = false
        model.onSelectRetryFee()
        try await waitFor { model.isConfirmEnabled }
        #expect(service.inputs.count == 2)
        #expect(model.paymailResolutionState == .resolved)
        #expect(feeCount == 1)
    }

    @Test
    func paymailTransferIsDiscardedAfterLeavingSend() async throws {
        var response: CheckedContinuation<BsvHomeTransferData, any Error>?
        var feeCount = 0
        let service = BsvHomeSendServiceMock { _, _, _, _ in
            try await withCheckedThrowingContinuation { response = $0 }
        }
        let model = makeModel(service: service, feeLoader: { transfer in
            feeCount += 1
            return validFeeLoad(fee: 500, transfer: transfer)
        })
        setPaymailInputs(on: model, handle: "alice@example.com", amount: "0.1")
        try await waitFor { response != nil }
        model.onDisappear()
        #expect(model.paymailResolutionState == .none)
        response?.resume(returning: .mock(type: .transfer(Chain.bitcoinSv.asset), recipient: .mock(name: "alice@example.com", address: address), value: 10_000_000))
        try await Task.sleep(for: .milliseconds(20))
        #expect(feeCount == 0)
        #expect(!model.isConfirmEnabled)
    }

    @Test(arguments: [Chain.tron.asset, .mockTronUSDT()])
    func paymailCannotBeUsedForOtherAssets(asset: Asset) {
        let service = BsvHomeSendServiceMock()
        let model = makeTronModel(asset: asset, service: service) { _ in .mock() }
        setPaymailInputs(on: model, handle: "alice@example.com", amount: "0.1")
        #expect(model.recipientError == Localized.Errors.notSupported)
        #expect(!model.isConfirmEnabled)
        #expect(service.inputs.isEmpty)
    }

    @Test
    func paymailCannotBeUsedForBsvTokens() {
        let service = BsvHomeSendServiceMock()
        let model = makeModel(asset: .mock(id: AssetId(chain: .bitcoinSv, tokenId: "token")), service: service)
        setPaymailInputs(on: model, handle: "alice@example.com", amount: "0.1")
        #expect(model.recipientError == Localized.Errors.notSupported)
        #expect(!model.isConfirmEnabled)
        #expect(service.inputs.isEmpty)
    }

    @Test
    func invalidBsvRecipientShowsErrorAndCannotStartFeeEstimate() {
        var feeRequestCount = 0
        let model = makeModel(feeLoader: { _ in
            feeRequestCount += 1
            return .mock()
        })

        model.addressInputModel.text = "not-a-bsv-address"
        model.onChangeAddressText("", new: model.addressInputModel.text)
        model.amountInputModel.text = "0.1"
        model.onChangeAmountText("", "0.1")

        #expect(model.recipientError != nil)
        #expect(model.isConfirmEnabled == false)
        #expect(feeRequestCount == 0)
    }

    @Test
    func percentagesTruncateToSatoshisAndFullBalanceUsesMax() {
        let model = makeModel(balance: 123_456_789)

        model.onSelectPercentage(25)
        #expect(model.amountInputModel.text == "0.30864197")
        #expect(model.amountEntry.value == BigInt(30_864_197))
        #expect(model.amountEntry.isMax == false)

        model.onSelectPercentage(100)
        #expect(model.amountInputModel.text == "1.23456789")
        #expect(model.amountEntry.isMax)
    }

    @Test
    func maximumAmountPassesCoreMaxFlagAndFullAvailableSatoshisToFeeLoader() async throws {
        var observedTransfer: BsvHomeTransferData?
        let model = makeModel(balance: 123_456_789, feeLoader: { transfer in
            observedTransfer = transfer
            return validFeeLoad(fee: 500, transfer: transfer)
        })
        model.addressInputModel.update(text: address)
        model.onChangeAddressText("", new: address)
        model.onSelectMax()

        try await waitFor { observedTransfer != nil }
        #expect(observedTransfer?.useMaxAmount == true)
        #expect(observedTransfer?.value == "123456789")
    }

    @Test
    func failedFeeEstimateBlocksConfirmationUntilRetrySucceeds() async throws {
        var requestCount = 0
        var shouldFail = true
        let model = makeModel(feeLoader: { transfer in
            requestCount += 1
            if shouldFail {
                throw NSError(domain: "Fee unavailable", code: 1)
            }
            return validFeeLoad(fee: 500, transfer: transfer)
        })
        setValidInputs(on: model, amount: "0.1")

        try await waitFor { model.feeError != nil }
        #expect(requestCount == 1)
        #expect(model.isConfirmEnabled == false)
        #expect(model.hasFeeQuote == false)

        shouldFail = false
        model.onSelectRetryFee()
        try await waitFor { model.isConfirmEnabled }
        #expect(requestCount == 2)
        #expect(model.feeError == nil)
        #expect(model.hasFeeQuote)
        #expect(model.feeAmount != Localized.wowpaySendFeePending)
    }

    @Test
    func oldFeeResponseCannotReplaceNewAmountQuote() async throws {
        var firstResponse: CheckedContinuation<BsvHomeConfirmLoad, any Error>?
        var firstTransfer: BsvHomeTransferData?
        var requestCount = 0
        let model = makeModel(feeLoader: { transfer in
            requestCount += 1
            if requestCount == 1 {
                firstTransfer = transfer
                return try await withCheckedThrowingContinuation { firstResponse = $0 }
            }
            return validFeeLoad(fee: 200, transfer: transfer)
        })
        setValidInputs(on: model, amount: "0.1")
        try await waitFor { firstResponse != nil }

        model.amountInputModel.text = "0.2"
        model.onChangeAmountText("0.1", "0.2")
        #expect(model.isConfirmEnabled == false)
        #expect(model.hasFeeQuote == false)
        try await waitFor { model.isConfirmEnabled }
        let freshFee = model.feeAmount
        #expect(requestCount == 2)

        if let firstTransfer {
            firstResponse?.resume(returning: validFeeLoad(fee: 900, transfer: firstTransfer))
        }
        try await Task.sleep(for: .milliseconds(20))
        #expect(model.feeAmount == freshFee)
        #expect(model.isConfirmEnabled)
    }

    @Test(arguments: [Chain.tron.asset, .mockTronUSDT()])
    func tronTransfersUseSunPrecisionAndTronRecipientValidation(asset: Asset) {
        var feeRequestCount = 0
        let model = makeTronModel(asset: asset) { _ in
            feeRequestCount += 1
            return .mock()
        }

        #expect(model.addressInputModel.chain == .tron)
        model.onSelectPercentage(25)
        #expect(model.amountInputModel.text == "0.308641")
        #expect(model.amountEntry.value == BigInt(308_641))

        model.addressInputModel.update(text: address)
        model.onChangeAddressText("", new: address)
        #expect(model.recipientError != nil)
        #expect(model.isConfirmEnabled == false)
        #expect(feeRequestCount == 0)
    }

    @Test
    func tronUsdtQuotesTrxFeeBeforeOpeningExistingConfirmation() async throws {
        var confirmedTransfer: BsvHomeTransferData?
        var quotedTransfer: BsvHomeTransferData?
        let model = makeTronModel(onTransferAction: { confirmedTransfer = $0 }) { transfer in
            quotedTransfer = transfer
            return tronFeeLoad(fee: 1_500_000, transfer: transfer)
        }
        model.addressInputModel.update(text: tronAddress)
        model.onChangeAddressText("", new: tronAddress)
        model.amountInputModel.text = "0.5"
        model.onChangeAmountText("", "0.5")

        try await waitFor { model.isConfirmEnabled }
        #expect(quotedTransfer?.value == "500000")
        #expect(model.feeAmount.contains("TRX"))
        #expect(model.feeAssetSymbol == "TRX")
        model.onSelectConfirm()
        #expect(confirmedTransfer?.value == quotedTransfer?.value)
        #expect(confirmedTransfer?.feeAsset().map().id == Chain.tron.asset.id)
    }

    @Test(arguments: [Chain.tron.asset, .mockTronUSDT()])
    func tronTransfersRejectQuoteWithTokenAsFeeAsset(asset: Asset) async throws {
        let model = makeTronModel(asset: asset) { transfer in
            let token = Asset.mockTronUSDT()
            return .mock(
                sender: .mock(chain: .tron, address: tronAddress),
                feeAsset: token,
                preload: .mock(confirmData: .mock(
                    input: BsvHomeConfirmInput(from: Account.mock(chain: .tron, address: tronAddress).map(), transfer: transfer),
                    fee: .mock(fee: 1_500_000, feeAsset: token.id.identifier),
                )),
            )
        }
        model.addressInputModel.update(text: tronAddress)
        model.onChangeAddressText("", new: tronAddress)
        model.amountInputModel.text = "0.5"
        model.onChangeAmountText("", "0.5")

        try await waitFor { model.feeError != nil }
        #expect(model.isConfirmEnabled == false)
        #expect(model.hasFeeQuote == false)
    }

    @Test
    func tronUsdtRejectsQuoteForDifferentTransfer() async throws {
        let model = makeTronModel { _ in
            tronFeeLoad(fee: 1_500_000, transfer: .mock())
        }
        model.addressInputModel.update(text: tronAddress)
        model.onChangeAddressText("", new: tronAddress)
        model.amountInputModel.text = "0.5"
        model.onChangeAmountText("", "0.5")

        try await waitFor { model.feeError != nil }
        #expect(model.isConfirmEnabled == false)
        #expect(model.hasFeeQuote == false)
    }

    @Test
    func tronMaximumDisplaysTheCoreNetAmountAndKeepsMaxForConfirmation() async throws {
        var confirmedTransfer: BsvHomeTransferData?
        var quotedTransfer: BsvHomeTransferData?
        let model = makeTronModel(asset: Chain.tron.asset, balance: 12_345_678, price: .mock(price: 1), onTransferAction: { confirmedTransfer = $0 }) { transfer in
            quotedTransfer = transfer
            return tronFeeLoad(
                fee: 1_500_000,
                transfer: transfer,
                amount: .amount(amount: .mock(value: 10_845_678, networkFee: 1_500_000, isMaxAmount: true)),
            )
        }
        model.addressInputModel.update(text: tronAddress)
        model.onChangeAddressText("", new: tronAddress)
        model.onSelectMax()

        #expect(model.amountInputModel.text == "12.345678")
        #expect(!model.isConfirmEnabled)
        try await waitFor { model.isConfirmEnabled }
        #expect(quotedTransfer?.inputAsset().map().id == Chain.tron.asset.id)
        #expect(quotedTransfer?.value == "12345678")
        #expect(quotedTransfer?.useMaxAmount == true)
        #expect(model.amountEquivalent == "≈ $10.845678")
        #expect(model.feeAssetSymbol == "TRX")
        model.onSelectConfirm()
        #expect(confirmedTransfer == quotedTransfer)
    }

    @Test(arguments: [Chain.tron.asset, .mockTronUSDT()])
    func tronInsufficientFeeResultBlocksConfirmation(asset: Asset) async throws {
        let model = makeTronModel(asset: asset) { transfer in
            tronFeeLoad(
                fee: 1_500_000,
                transfer: transfer,
                amount: .error(error: .InsufficientNetworkFee(asset: Chain.tron.asset.map(), requirement: nil)),
            )
        }
        model.addressInputModel.update(text: tronAddress)
        model.onChangeAddressText("", new: tronAddress)
        model.amountInputModel.text = "0.5"
        model.onChangeAmountText("", "0.5")

        try await waitFor { model.feeError != nil }
        #expect(!model.isConfirmEnabled)
        #expect(!model.hasFeeQuote)
    }

    private func setValidInputs(on model: WowPaySendViewModel, amount: String) {
        model.addressInputModel.update(text: address)
        model.onChangeAddressText("", new: address)
        model.amountInputModel.text = amount
        model.onChangeAmountText("", amount)
    }

    private func setPaymailInputs(on model: WowPaySendViewModel, handle: String, amount: String) {
        model.addressInputModel.text = handle
        model.onChangeAddressText("", new: handle)
        model.amountInputModel.text = amount
        model.onChangeAmountText("", amount)
    }

    private func validFeeLoad(fee: BigInt, transfer: BsvHomeTransferData) -> BsvHomeConfirmLoad {
        let asset = Chain.bitcoinSv.asset
        return .mock(
            sender: .mock(chain: .bitcoinSv, address: address),
            feeAsset: asset,
            preload: .mock(confirmData: .mock(
                input: BsvHomeConfirmInput(from: Account.mock(chain: .bitcoinSv, address: address).map(), transfer: transfer),
                fee: .mock(fee: fee, feeAsset: asset.id.identifier),
            )),
        )
    }

    private func tronFeeLoad(fee: BigInt, transfer: BsvHomeTransferData, amount: BsvHomeTransferAmountResult = .amount(amount: .mock(value: 1, networkFee: 1))) -> BsvHomeConfirmLoad {
        let feeAsset = Chain.tron.asset
        return .mock(
            sender: .mock(chain: .tron, address: tronAddress),
            feeAsset: feeAsset,
            preload: .mock(
                confirmData: .mock(
                    input: BsvHomeConfirmInput(from: Account.mock(chain: .tron, address: tronAddress).map(), transfer: transfer),
                    fee: .mock(fee: fee, feeAsset: feeAsset.id.identifier),
                ),
                amount: amount,
            ),
        )
    }

    private func waitFor(_ condition: @MainActor () -> Bool) async throws {
        for _ in 0 ..< 100 {
            if condition() {
                return
            }
            try await Task.sleep(for: .milliseconds(10))
        }
        Issue.record("Timed out waiting for send form state")
    }

    private func makeModel(
        asset: Asset = Chain.bitcoinSv.asset,
        balance: BigInt = 200_000_000,
        autoPrepare: Bool = true,
        service: BsvHomeSendServiceMock = BsvHomeSendServiceMock(),
        onTransferAction: TransferDataAction = nil,
        onManagePaymail: VoidAction = nil,
        prepareAddresses: @escaping @MainActor () async throws -> Void = {},
        feeLoader: @escaping @MainActor (BsvHomeTransferData) async throws -> BsvHomeConfirmLoad = { _ in .mock() },
    ) -> WowPaySendViewModel {
        let account = Account.mock(chain: .bitcoinSv, address: address)
        let wallet = Wallet.mock(accounts: [account])
        let assetData = AssetData.mock(
            asset: asset,
            balance: .mock(available: balance),
            account: account,
        )
        let model = WowPaySendViewModel(
            wallet: wallet,
            assetData: assetData,
            service: service,
            nameService: BsvHomeNameService.mock(),
            prepareBsvAddresses: prepareAddresses,
            feeLoader: feeLoader,
            onTransferAction: onTransferAction,
            onManagePaymail: onManagePaymail,
        )
        if autoPrepare {
            Task { await model.prepareBsvAddresses() }
        }
        return model
    }

    private func makeTronModel(
        asset: Asset = .mockTronUSDT(),
        balance: BigInt = 1_234_567,
        price: Price? = nil,
        onTransferAction: ((BsvHomeTransferData) -> Void)? = nil,
        service: BsvHomeSendServiceMock = BsvHomeSendServiceMock(),
        feeLoader: @escaping @MainActor (BsvHomeTransferData) async throws -> BsvHomeConfirmLoad,
    ) -> WowPaySendViewModel {
        let account = Account.mock(chain: .tron, address: tronAddress)
        let wallet = Wallet.mock(accounts: [account])
        let assetData = AssetData.mock(
            asset: asset,
            balance: .mock(available: balance),
            account: account,
            price: price,
        )
        return WowPaySendViewModel(
            wallet: wallet,
            assetData: assetData,
            service: service,
            nameService: BsvHomeNameService.mock(),
            prepareBsvAddresses: { Issue.record("Non-BSV send requested BSV authentication") },
            feeLoader: feeLoader,
            onTransferAction: onTransferAction,
        )
    }
}
