// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Components
import Formatters
import Foundation
import struct BsvHomeCore.BsvHomeAmountEntry
import struct BsvHomeCore.BsvHomeAmountInput
import enum BsvHomeCore.BsvHomeAmountType
import struct BsvHomeCore.BsvHomeAssetBalance
import enum BsvHomeCore.BsvHomeConfirmError
import struct BsvHomeCore.BsvHomeConfirmLoad
import protocol BsvHomeCore.BsvHomeNameServiceProtocol
import enum BsvHomeCore.BsvHomePaymailError
import struct BsvHomeCore.BsvHomeRecipient
import struct BsvHomeCore.BsvHomeSendContactRecipient
import class BsvHomeCore.BsvHomeSendPublicFundingPreparation
import protocol BsvHomeCore.BsvHomeSendServiceProtocol
import struct BsvHomeCore.BsvHomeSendWalletRecipient
import struct BsvHomeCore.BsvHomeTransferData
import BsvHomeCorePrimitives
import Localization
import Primitives
import PrimitivesComponents
import Store
import Validators

@Observable
@MainActor
public final class WowPaySendViewModel {
    enum PaymailResolutionState: Equatable {
        case none
        case resolving
        case resolved
    }

    private enum DestinationQuote {
        case transfer(BsvHomeTransferData)
        case publicFunding(BsvHomeSendPublicFundingPreparation, exactValue: BigInt)
    }

    private struct QuoteInput: Equatable {
        let recipient: String
        let amount: String
        let assetId: AssetId
        let account: Account
    }

    public let wallet: Wallet
    public let asset: Asset
    public let assetQuery: ObservableQuery<AssetRequest>
    public let contactsQuery: ObservableQuery<ContactsRequest>
    public let walletsQuery = ObservableQuery(WalletsRequest(isPinned: nil), initialValue: [Wallet]())

    public var addressInputModel: AddressInputViewModel
    public var amountInputModel = InputValidationViewModel(mode: .manual)
    public var isPresentingScanner = false
    public var isPresentingContacts = false

    private let service: any BsvHomeSendServiceProtocol
    private let prepareAddresses: @MainActor () async throws -> Void
    private let feeLoader: @MainActor (BsvHomeTransferData) async throws -> BsvHomeConfirmLoad
    private let onTransferAction: TransferDataAction
    private let managePaymail: VoidAction
    private let onPublicFunding: (@MainActor (BsvHomeSendPublicFundingPreparation) -> Void)?
    private let formatter = ValueFormatter(style: .full)

    private var selectedRecipientName: String?
    private var selectedRecipientAddress: String?
    private var destinationQuote: DestinationQuote?
    private var isRetired = false
    private var quotedInput: QuoteInput?
    private var resolvedPaymailInput: QuoteInput?
    private var quotedFee: BigInt?
    private var quotedFeeAsset: Asset?
    private var quotedAmount: BigInt?
    private var quotedFeePrice: Price?
    private(set) var feeTask: Task<Void, Never>?
    private var quoteRevision: UInt = 0
    private var contactsRevision: UInt = 0
    private var walletsRevision: UInt = 0
    private var paymailError: BsvHomePaymailError?
    private var addressesReady: Bool

    private(set) var addressesState: StateViewType<Void> = .noData
    private(set) var isEstimatingFee = false
    private(set) var feeError: String?
    private(set) var recipientError: String?
    private(set) var contactRecipients: [BsvHomeSendContactRecipient] = []
    private(set) var walletRecipients: [BsvHomeSendWalletRecipient] = []
    private(set) var walletRecipientsState: StateViewType<Void> = .noData

    public init(
        wallet: Wallet,
        assetData: AssetData,
        service: any BsvHomeSendServiceProtocol,
        nameService: any BsvHomeNameServiceProtocol,
        prepareBsvAddresses: @escaping @MainActor () async throws -> Void,
        feeLoader: @escaping @MainActor (BsvHomeTransferData) async throws -> BsvHomeConfirmLoad,
        onTransferAction: TransferDataAction,
        onManagePaymail: VoidAction = nil,
        onPublicFunding: (@MainActor (BsvHomeSendPublicFundingPreparation) -> Void)? = nil,
    ) {
        self.wallet = wallet
        asset = assetData.asset
        self.service = service
        prepareAddresses = prepareBsvAddresses
        addressesReady = assetData.asset.chain != .bitcoinSv
        self.feeLoader = feeLoader
        self.onTransferAction = onTransferAction
        managePaymail = onManagePaymail
        self.onPublicFunding = onPublicFunding
        assetQuery = ObservableQuery(AssetRequest(walletId: wallet.id, assetId: assetData.asset.id), initialValue: assetData)
        contactsQuery = ObservableQuery(ContactsRequest(chain: asset.chain), initialValue: [])
        addressInputModel = AddressInputViewModel(
            chain: asset.chain,
            nameService: nameService,
            placeholder: asset.chain == .bitcoinSv
                ? Localized.wowpaySendRecipientPlaceholder
                : Localized.wowpaySendAddressPlaceholder(asset.chain.networkName),
        )
    }

    public var assetData: AssetData { assetQuery.value }

    public func refreshContactRecipients() async {
        contactsRevision &+= 1
        let current = contactsRevision
        let asset = asset.map()
        let contacts = contactsQuery.value.map { $0.contact.map() }
        let addresses = contactsQuery.value.flatMap(\.addresses).map { $0.map() }
        let service = service
        do {
            let rows = try await Task.detached(priority: .userInitiated) { try service.contactRecipients(asset: asset, contacts: contacts, addresses: addresses) }.value
            guard current == contactsRevision, !Task.isCancelled else { return }
            contactRecipients = rows
        } catch {
            guard current == contactsRevision, !Task.isCancelled else { return }
            contactRecipients = []
            recipientError = Localized.Errors.invalidAssetAddress(self.asset.name)
        }
    }

    public func refreshWalletRecipients(wallets: [Wallet]) async {
        walletsRevision &+= 1
        let revision = walletsRevision
        walletRecipientsState = .loading
        walletRecipients = []
        let service = service
        let asset = asset.map()
        let sourceWalletId = wallet.id.id
        let wallets = wallets.map { $0.map() }
        do {
            let rows = try await Task.detached(priority: .userInitiated) {
                try service.walletRecipients(asset: asset, wallets: wallets, sourceWalletId: sourceWalletId)
            }.value
            guard revision == walletsRevision, !Task.isCancelled else { return }
            walletRecipients = rows
            walletRecipientsState = .data(())
        } catch {
            guard revision == walletsRevision, !Task.isCancelled else { return }
            walletRecipientsState = .error(error)
        }
    }

    var priceModel: PriceViewModel {
        PriceViewModel(price: assetData.price, currencyCode: service.getCurrency())
    }

    var amountEquivalent: String {
        guard let value = quotedAmount ?? amountEntry.value,
              let valueText = priceModel.fiatAmountText(value: value, decimals: asset.decimals.asInt)
        else { return Localized.Common.notAvailable }
        return "≈ \(valueText)"
    }

    var availableBalance: String {
        let value = formatter.balanceString(assetData.balance.available, asset: asset, currency: asset.symbol)
        return Localized.wowpaySendAvailableBalance(value)
    }

    var availableBalanceEquivalent: String {
        guard let amount = priceModel.fiatAmountText(value: assetData.balance.available, decimals: asset.decimals.asInt) else {
            return Localized.Common.notAvailable
        }
        return "≈ \(amount)"
    }

    var feeAmount: String {
        guard let quotedFee, let quotedFeeAsset else { return Localized.wowpaySendFeePending }
        return formatter.string(quotedFee, asset: quotedFeeAsset)
    }

    var hasFeeQuote: Bool { quotedFee != nil && quotedFeeAsset != nil }

    var feeAssetSymbol: String {
        quotedFeeAsset?.symbol ?? asset.chain.asset.symbol
    }

    var feeEquivalent: String {
        guard let quotedFee, let quotedFeeAsset,
              let amount = PriceViewModel(price: quotedFeePrice, currencyCode: service.getCurrency())
              .fiatAmountText(value: quotedFee, decimals: quotedFeeAsset.decimals.asInt)
        else { return Localized.Common.notAvailable }
        return "≈ \(amount)"
    }

    var isConfirmEnabled: Bool {
        guard !isRetired, addressesReady, quotedInput == quoteInput, !isEstimatingFee, feeError == nil, recipientError == nil else { return false }
        switch destinationQuote {
        case .transfer: return hasFeeQuote
        case let .publicFunding(_, exactValue):
            return onPublicFunding != nil && amountEntry.error == nil && amountEntry.value == exactValue
        case nil: return false
        }
    }

    var isAddressPreparationComplete: Bool { addressesReady }

    var isPaymailInput: Bool {
        service.isPaymailInput(input: addressInputModel.text)
    }

    var supportsPaymail: Bool { service.supportsPaymail(asset: asset.map()) }

    var requiresPaymailSender: Bool {
        if case .SenderRequired? = paymailError {
            true
        } else {
            false
        }
    }

    var canManagePaymailSender: Bool { requiresPaymailSender && managePaymail != nil }

    var paymailResolutionState: PaymailResolutionState {
        guard isPaymailInput, supportsPaymail, recipientError == nil else { return .none }
        if resolvedPaymailInput == quoteInput {
            return .resolved
        }
        return isEstimatingFee ? .resolving : .none
    }

    var amountError: String? {
        guard amountInputModel.text.isNotEmpty,
              let error = amountInputModel.error,
              !(error is SilentValidationError)
        else { return nil }
        return error.localizedDescription
    }

    var amountEntry: BsvHomeAmountEntry {
        BsvHomeAmountType.transfer.entry(
            asset: asset.map(),
            input: amountInput,
            price: assetData.price?.price,
            inputType: .asset,
            text: formatter.plainInputNumber(amountInputModel.text),
        )
    }

    private var amountInput: BsvHomeAmountInput {
        BsvHomeAmountType.transfer.input(
            asset: asset.map(),
            balance: BsvHomeAssetBalance(assetData.balance, assetId: asset.id, isActive: assetData.metadata.isActive),
        )
    }

    public func onChangeAddressText(_: String, new: String) {
        if new != selectedRecipientAddress {
            selectedRecipientName = nil
            selectedRecipientAddress = nil
        }
        if isPaymailInput {
            addressInputModel.update(error: nil)
        } else {
            _ = addressInputModel.update()
        }
        refreshFee()
    }

    public func onChangeAmountText(_: String, _: String) {
        updateAmountError()
        refreshFee()
    }

    public func onChangeAssetData(_: AssetData, _: AssetData) {
        updateAmountError()
        refreshFee()
    }

    func onSelectPercentage(_ percentage: Int) {
        guard (1 ..< 100).contains(percentage) else {
            onSelectMax()
            return
        }
        let value = amountInput.availableValue * BigInt(percentage) / 100
        amountInputModel.text = formatter.inputString(value, decimals: asset.decimals.asInt)
        onChangeAmountText("", amountInputModel.text)
    }

    func onSelectMax() {
        let max = amountInput.maxEntry()
        amountInputModel.text = formatter.inputString(max.value, decimals: asset.decimals.asInt)
        onChangeAmountText("", amountInputModel.text)
    }

    public func onSelectRetryFee() {
        refreshFee()
    }

    func onSelectContact(_ contact: BsvHomeSendContactRecipient) {
        onSelectRecipient(contact.recipient)
    }

    func onSelectWallet(_ wallet: BsvHomeSendWalletRecipient) {
        onSelectRecipient(wallet.recipient)
    }

    private func onSelectRecipient(_ recipient: BsvHomeRecipient) {
        selectedRecipientName = recipient.name
        selectedRecipientAddress = recipient.address
        addressInputModel.update(text: recipient.address)
        onChangeAddressText("", new: recipient.address)
        isPresentingContacts = false
    }

    func onSelectManagePaymail() {
        guard canManagePaymailSender else { return }
        invalidateFeeQuote()
        managePaymail?()
    }

    public func onHandleScan(_ value: String) {
        do {
            let payment = try service.scan(asset: asset.map(), input: value)
            if let amount = payment.amount {
                let entry = BsvHomeAmountType.transfer.entry(
                    asset: asset.map(),
                    input: amountInput,
                    price: assetData.price?.price,
                    inputType: .asset,
                    text: formatter.plainInputNumber(amount),
                )
                guard let value = entry.value, value > 0, entry.error == nil else {
                    throw BsvHomeConfirmError.ScanMalicious
                }
                amountInputModel.text = formatter.inputString(value, decimals: asset.decimals.asInt)
                onChangeAmountText("", amountInputModel.text)
            }
            addressInputModel.update(text: payment.recipient.address)
            onChangeAddressText("", new: payment.recipient.address)
        } catch {
            addressInputModel.update(text: "")
            onChangeAddressText("", new: "")
            recipientError = Localized.Errors.invalidAssetAddress(asset.name)
        }
    }

    func onSelectConfirm() {
        guard isConfirmEnabled else { return }
        switch destinationQuote {
        case let .transfer(transfer):
            onTransferAction?(transfer)
        case let .publicFunding(preparation, _):
            guard let onPublicFunding else { return }
            destinationQuote = nil
            quotedInput = nil
            resolvedPaymailInput = nil
            onPublicFunding(preparation)
        case nil: break
        }
    }

    func onDisappear() {
        invalidateFeeQuote()
        walletsRevision &+= 1
        walletRecipients = []
        walletRecipientsState = .noData
    }

    public func cancel() {
        isRetired = true
        onDisappear()
        contactsRevision &+= 1
    }

    func prepareBsvAddresses() async {
        guard !isRetired, !Task.isCancelled, !addressesReady, !addressesState.isLoading else { return }
        addressesState = .loading
        do {
            try await prepareAddresses()
            try Task.checkCancellation()
            guard !isRetired else { return }
            addressesReady = true
            addressesState = .data(())
            refreshFee()
        } catch {
            guard !isRetired else { return }
            addressesState = .error(error)
        }
    }
}

private extension WowPaySendViewModel {
    func invalidateFeeQuote() {
        quoteRevision &+= 1
        feeTask?.cancel()
        feeTask = nil
        if case let .publicFunding(preparation, _) = destinationQuote {
            preparation.cancel()
        }
        destinationQuote = nil
        quotedInput = nil
        resolvedPaymailInput = nil
        isEstimatingFee = false
    }

    func updateAmountError() {
        switch amountEntry.error {
        case .none:
            amountInputModel.update(error: nil)
        case .Zero:
            amountInputModel.update(error: SilentValidationError())
        case let .some(error):
            amountInputModel.update(error: error)
        }
    }

    private var quoteInput: QuoteInput {
        QuoteInput(recipient: addressInputModel.text, amount: amountInputModel.text, assetId: assetData.asset.id, account: assetData.account)
    }

    func refreshFee() {
        guard !isRetired else { return }
        invalidateFeeQuote()
        let revision = quoteRevision
        quotedFee = nil
        quotedFeeAsset = nil
        quotedAmount = nil
        quotedFeePrice = nil
        feeError = nil
        recipientError = if isPaymailInput {
            supportsPaymail ? nil : Localized.Errors.notSupported
        } else if addressInputModel.text.isNotEmpty, !addressInputModel.isValid {
            Localized.Errors.invalidAssetAddress(asset.name)
        } else {
            nil
        }
        paymailError = nil

        guard addressesReady, recipientError == nil,
              amountEntry.error == nil,
              let value = amountEntry.value, value > 0
        else { return }

        let input = quoteInput
        guard isPaymailInput || addressInputModel.isValid else { return }

        let useMaxAmount = amountEntry.isMax
        let transferAsset = asset
        let senderAddress = assetData.account.address
        let sendService = service
        let nameState = addressInputModel.nameResolveState
        let contactName = selectedRecipientName
        let loadFee = feeLoader

        isEstimatingFee = true
        feeTask = Task { [weak self] in
            defer {
                if self?.quoteRevision == revision {
                    self?.feeTask = nil
                    self?.isEstimatingFee = false
                }
            }
            do {
                try await Task.sleep(for: .milliseconds(250))
                let destination = try await sendService.resolveDestination(
                    asset: transferAsset.map(),
                    input: input.recipient,
                    state: nameState,
                    name: contactName,
                    value: value,
                    useMaxAmount: useMaxAmount,
                )
                let transfer: BsvHomeTransferData
                switch destination {
                case let .publicFunding(preparation):
                    guard !Task.isCancelled, let self, !isRetired, quoteRevision == revision, input == quoteInput else {
                        preparation.cancel()
                        return
                    }
                    guard onPublicFunding != nil else {
                        preparation.cancel()
                        recipientError = Localized.Errors.notSupported
                        return
                    }
                    destinationQuote = .publicFunding(preparation, exactValue: value)
                    quotedInput = input
                    resolvedPaymailInput = input
                    return
                case let .transfer(resolved):
                    guard !Task.isCancelled, self?.isRetired == false, self?.quoteRevision == revision else { return }
                    transfer = resolved
                }
                if self?.isPaymailInput == true {
                    self?.resolvedPaymailInput = input
                }
                let load = try await loadFee(transfer)
                guard !Task.isCancelled, let self, !isRetired, revision == quoteRevision else { return }
                guard load.sender.chain == transferAsset.chain.map(),
                      load.sender.address == senderAddress,
                      load.feeAsset.map().id == transfer.feeAsset().map().id,
                      let preload = load.preload,
                      preload.confirmData.input.transfer == transfer,
                      AssetId(core: preload.confirmData.fee.feeAsset) == load.feeAsset.map().id
                else {
                    feeError = Localized.Errors.unableEstimateNetworkFee
                    return
                }
                switch preload.amount {
                case let .amount(amount):
                    destinationQuote = .transfer(transfer)
                    quotedInput = input
                    quotedFee = preload.confirmData.fee.fee
                    quotedFeeAsset = load.feeAsset.map()
                    quotedAmount = amount.value
                    quotedFeePrice = load.metadata.feePrice
                case let .error(error):
                    feeError = error.localizedDescription
                }
            } catch {
                guard !Task.isCancelled, let self, !isRetired, revision == quoteRevision else { return }
                if let error = error as? BsvHomePaymailError {
                    paymailError = error
                    recipientError = if case .SenderRequired = error {
                        Localized.wowpayPaymailSenderRequired
                    } else {
                        error.localizedDescription
                    }
                } else {
                    feeError = error.localizedDescription
                }
            }
        }
    }
}
