import BigInt
import Foundation
import struct BsvHomeCore.BsvHomeConfirmInput
import struct BsvHomeCore.BsvHomeConfirmLoad
import class BsvHomeCore.BsvHomeNameService
import enum BsvHomeCore.BsvHomePaymailError
import enum BsvHomeCore.BsvHomeSendDestination
import class BsvHomeCore.BsvHomeSendPublicFundingPreparation
import protocol BsvHomeCore.BsvHomeSendServiceProtocol
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
struct WowPaySendPublicFundingTests {
    @Test
    func ordinaryPaymailKeepsTheTransferFeeAndMaximumAmount() async throws {
        let fixture = try await SendPublicPaymailFixture.make(output: .p2pkh)
        let service = BsvHomeSendServiceMock()
        var maximum: Bool?
        var feeCalls = 0
        var transfer: BsvHomeTransferData?
        var publicCalls = 0
        service.onResolveDestination = { _, _, value, useMaxAmount in
            maximum = useMaxAmount
            return try await fixture.destination(value: value, useMaxAmount: useMaxAmount)
        }
        let model = try await makeModel(fixture: fixture, service: service, onTransfer: { transfer = $0 }, onPublicFunding: { _ in publicCalls += 1 }, feeLoader: { transfer in
            feeCalls += 1
            return feeLoad(transfer: transfer, wallet: fixture.wallet.map())
        })
        setRecipient(on: model)
        model.onSelectMax()
        try await (#require(model.feeTask)).value

        #expect(model.isConfirmEnabled)
        #expect(model.hasFeeQuote)
        #expect(maximum == true)
        model.onSelectConfirm()
        #expect(transfer?.useMaxAmount == true)
        #expect(transfer?.value == "2000000")
        #expect(transfer?.recipient.name == "receiver@example.invalid")
        #expect(feeCalls == 1)
        #expect(publicCalls == 0)
        #expect(service.inputs == ["receiver@example.invalid"])
    }

    @Test
    func publicPaymailMaximumHandsOffTheExactAmountWithoutAnOrdinaryFee() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let service = BsvHomeSendServiceMock()
        var feeCalls = 0
        var ordinaryCalls = 0
        var adopted: BsvHomeSendPublicFundingPreparation?
        service.onResolveDestination = { _, _, value, useMaxAmount in
            #expect(useMaxAmount)
            return try await fixture.destination(value: value, useMaxAmount: useMaxAmount)
        }
        let model = try await makeModel(fixture: fixture, service: service, onTransfer: { _ in ordinaryCalls += 1 }, onPublicFunding: { adopted = $0 }, feeLoader: { _ in
            feeCalls += 1
            return .mock()
        })
        setRecipient(on: model)
        model.onSelectMax()
        try await (#require(model.feeTask)).value

        #expect(model.isConfirmEnabled)
        #expect(!model.hasFeeQuote)
        #expect(model.feeAmount == Localized.wowpaySendFeePending)
        model.onSelectConfirm()
        let preparation = try #require(adopted)
        #expect(preparation.paymentRequest().amount == "0.02")
        let final = try await fixture.service.confirmPublicFunding(preparation: preparation, value: 2_000_000)
        #expect(final.amount == "0.02")
        #expect(feeCalls == 0)
        #expect(ordinaryCalls == 0)
        #expect(fixture.credentialAccessCount == 0)
        #expect(fixture.rejectedRequestCount == 0)
    }

    @Test
    func aLatePublicQuoteCannotReplaceOrCancelTheNewPreparation() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let old = try await fixture.prepare(value: 42000)
        let new = try await fixture.prepare(value: 84000)
        let pending = AsyncStream<CheckedContinuation<BsvHomeSendDestination, any Error>>.makeStream()
        var responses = pending.stream.makeAsyncIterator()
        let service = BsvHomeSendServiceMock()
        var requests = 0
        var adopted: BsvHomeSendPublicFundingPreparation?
        service.onResolveDestination = { _, _, _, _ in
            requests += 1
            if requests == 1 {
                return try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
            }
            return .publicFunding(preparation: new)
        }
        let model = try await makeModel(fixture: fixture, service: service, onPublicFunding: { adopted = $0 })
        setInputs(on: model, amount: "0.00042")
        let oldTask = try #require(model.feeTask)
        let response = try #require(await responses.next())
        setAmount(on: model, amount: "0.00084")
        try await (#require(model.feeTask)).value
        response.resume(returning: .publicFunding(preparation: old))
        await oldTask.value

        #expect(model.isConfirmEnabled)
        model.onSelectConfirm()
        #expect(adopted?.id() == new.id())
        await #expect(throws: BsvHomePaymailError.Auth(error: .Service(error: .Cancelled))) {
            try await fixture.service.confirmPublicFunding(preparation: old, value: 42000)
        }
        let final = try await fixture.service.confirmPublicFunding(preparation: new, value: 84000)
        #expect(final.amount == "0.00084")
        #expect(requests == 2)
    }

    @Test
    func editingAResolvedQuoteRetiresOnlyItsOwnedPreparation() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let old = try await fixture.prepare(value: 42000)
        let new = try await fixture.prepare(value: 84000)
        let service = BsvHomeSendServiceMock()
        service.onResolveDestination = { _, _, value, _ in .publicFunding(preparation: value == 42000 ? old : new) }
        let model = try await makeModel(fixture: fixture, service: service, onPublicFunding: { _ in })
        setInputs(on: model, amount: "0.00042")
        try await (#require(model.feeTask)).value
        setAmount(on: model, amount: "0.00084")
        let newTask = try #require(model.feeTask)

        await #expect(throws: BsvHomePaymailError.Auth(error: .Service(error: .Cancelled))) {
            try await fixture.service.confirmPublicFunding(preparation: old, value: 42000)
        }
        await newTask.value
        #expect(model.isConfirmEnabled)
        let final = try await fixture.service.confirmPublicFunding(preparation: new, value: 84000)
        #expect(final.amount == "0.00084")
        model.cancel()
    }

    @Test(arguments: [false, true])
    func anAdoptedPreparationSurvivesTheSourcePageExit(permanent: Bool) async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let service = BsvHomeSendServiceMock()
        var calls = 0
        weak var source: WowPaySendViewModel?
        service.onResolveDestination = { _, _, _, _ in .publicFunding(preparation: preparation) }
        let model = try await makeModel(fixture: fixture, service: service, onPublicFunding: { adopted in
            #expect(adopted.id() == preparation.id())
            calls += 1
            if permanent {
                source?.cancel()
            } else {
                source?.onDisappear()
            }
        })
        source = model
        setInputs(on: model, amount: "0.00042")
        try await (#require(model.feeTask)).value
        model.onSelectConfirm()
        model.onSelectConfirm()

        let final = try await fixture.service.confirmPublicFunding(preparation: preparation, value: 42000)
        #expect(final.amount == "0.00042")
        #expect(calls == 1)
        #expect(!model.isConfirmEnabled)
    }

    @Test
    func trueExitRetiresALatePreparationWithoutNavigating() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let pending = AsyncStream<CheckedContinuation<BsvHomeSendDestination, any Error>>.makeStream()
        var responses = pending.stream.makeAsyncIterator()
        let service = BsvHomeSendServiceMock()
        var calls = 0
        service.onResolveDestination = { _, _, _, _ in
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let model = try await makeModel(fixture: fixture, service: service, onPublicFunding: { _ in calls += 1 })
        setInputs(on: model, amount: "0.00042")
        let task = try #require(model.feeTask)
        let response = try #require(await responses.next())
        model.cancel()
        response.resume(returning: .publicFunding(preparation: preparation))
        await task.value
        model.onSelectConfirm()

        #expect(calls == 0)
        #expect(!model.isConfirmEnabled)
        await #expect(throws: BsvHomePaymailError.Auth(error: .Service(error: .Cancelled))) {
            try await fixture.service.confirmPublicFunding(preparation: preparation, value: 42000)
        }
    }

    @Test
    func aMissingPublicHandlerRejectsAndCancelsThePreparation() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let service = BsvHomeSendServiceMock()
        service.onResolveDestination = { _, _, _, _ in .publicFunding(preparation: preparation) }
        let model = try await makeModel(fixture: fixture, service: service)
        setInputs(on: model, amount: "0.00042")
        try await (#require(model.feeTask)).value

        #expect(model.recipientError == Localized.Errors.notSupported)
        #expect(!model.isConfirmEnabled)
        #expect(!model.hasFeeQuote)
        await #expect(throws: BsvHomePaymailError.Auth(error: .Service(error: .Cancelled))) {
            try await fixture.service.confirmPublicFunding(preparation: preparation, value: 42000)
        }
    }

    @Test(arguments: ["0.00084", "0", "0.000000001"])
    func anUnobservedAmountEditCannotHandOffTheOldQuote(amount: String) async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let service = BsvHomeSendServiceMock()
        var calls = 0
        service.onResolveDestination = { _, _, _, _ in .publicFunding(preparation: preparation) }
        let model = try await makeModel(fixture: fixture, service: service, onPublicFunding: { _ in calls += 1 })
        setInputs(on: model, amount: "0.00042")
        try await (#require(model.feeTask)).value
        model.amountInputModel.text = amount
        model.onSelectConfirm()

        #expect(calls == 0)
        #expect(!model.isConfirmEnabled)
        model.cancel()
        await #expect(throws: BsvHomePaymailError.Auth(error: .Service(error: .Cancelled))) {
            try await fixture.service.confirmPublicFunding(preparation: preparation, value: 42000)
        }
    }

    @Test
    func finalAmountRevalidationUsesTheOriginalSourceAndReturnedRequest() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000, useMaxAmount: true)
        let other = try await SendPublicPaymailFixture.make()
        await #expect(throws: BsvHomePaymailError.BindingChanged) {
            try await other.service.confirmPublicFunding(preparation: preparation, value: 84000)
        }
        let final = try await fixture.service.confirmPublicFunding(preparation: preparation, value: 84000)
        #expect(final.amount == "0.00084")
        #expect(preparation.paymentRequest().amount == "0.00042")
        #expect(final.fingerprint == preparation.paymentRequest().fingerprint)

        fixture.setOutput(.p2pkh)
        await #expect(throws: BsvHomePaymailError.BindingChanged) {
            try await fixture.service.confirmPublicFunding(preparation: preparation, value: 84000)
        }
        #expect(fixture.credentialAccessCount == 0)
        #expect(other.credentialAccessCount == 0)
        #expect(fixture.rejectedRequestCount == 0)
    }

    private func makeModel(
        fixture: SendPublicPaymailFixture,
        service: any BsvHomeSendServiceProtocol,
        onTransfer: TransferDataAction = nil,
        onPublicFunding: (@MainActor (BsvHomeSendPublicFundingPreparation) -> Void)? = nil,
        feeLoader: @escaping @MainActor (BsvHomeTransferData) async throws -> BsvHomeConfirmLoad = { _ in
            Issue.record("Public destination requested an ordinary transfer fee")
            return .mock()
        },
    ) async throws -> WowPaySendViewModel {
        let wallet = fixture.wallet.map()
        let account = try #require(wallet.accounts.first)
        let model = WowPaySendViewModel(
            wallet: wallet,
            assetData: .mock(asset: fixture.asset.map(), balance: .mock(available: 2_000_000), account: account),
            service: service,
            nameService: BsvHomeNameService.mock(),
            prepareBsvAddresses: {},
            feeLoader: feeLoader,
            onTransferAction: onTransfer,
            onPublicFunding: onPublicFunding,
        )
        await model.prepareBsvAddresses()
        return model
    }

    private func feeLoad(transfer: BsvHomeTransferData, wallet: Wallet) -> BsvHomeConfirmLoad {
        let asset = Chain.bitcoinSv.asset
        let account = wallet.accounts[0]
        return .mock(
            sender: .mock(chain: .bitcoinSv, address: account.address),
            feeAsset: asset,
            preload: .mock(confirmData: .mock(
                input: BsvHomeConfirmInput(from: account.map(), transfer: transfer),
                fee: .mock(fee: 500, feeAsset: asset.id.identifier),
            )),
        )
    }

    private func setRecipient(on model: WowPaySendViewModel) {
        model.addressInputModel.update(text: "receiver@example.invalid")
        model.onChangeAddressText("", new: "receiver@example.invalid")
    }

    private func setAmount(on model: WowPaySendViewModel, amount: String) {
        let old = model.amountInputModel.text
        model.amountInputModel.text = amount
        model.onChangeAmountText(old, amount)
    }

    private func setInputs(on model: WowPaySendViewModel, amount: String) {
        setRecipient(on: model)
        setAmount(on: model, amount: amount)
    }
}
