import BigInt
@testable import BsvMultisig
import Components
import Foundation
import BsvHomeCore
import BsvHomeCorePrimitivesTestKit
import Localization
import Testing

@MainActor
struct BsvMultisigPublicFundingViewModelTests {
    @Test(.timeLimit(.minutes(1)))
    func repeatedAppearanceSharesTheOwnedRecoveryRead() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let service = BsvMultisigPublicFundingServiceMock()
        let gate = BsvMultisigAsyncGate()
        service.onResults = { await gate.wait(); return [] }
        let model = makeModel(service: service, payer: fixture.wallet)
        model.startLoading()
        let read = try #require(model.readTask)
        await gate.started()
        model.startLoading()
        model.reloadResults()
        #expect(service.calls == ["results"])
        await gate.release()
        await read.value
        #expect(model.recoveryState.value?.results.isEmpty == true)
        #expect(model.recoveryState.value?.unsubmitted.isEmpty == true)
        #expect(service.calls == ["results", "unsubmitted"])
        #expect(model.readTask == nil)
    }

    @Test
    func cancellationBeforeTheFirstReadPollConstructsNoService() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let service = BsvMultisigPublicFundingServiceMock()
        let model = BsvMultisigPublicFundingViewModel(entry: .recovery, payer: fixture.wallet, makeService: {
            service.recordFactoryCreation()
            return service
        })
        model.startLoading()
        let read = try #require(model.readTask)
        model.cancel()
        await read.value
        #expect(service.calls.isEmpty)
        #expect(service.factoryCreationCount == 0)
        #expect(model.readTask == nil)
        #expect(model.recoveryState.isNoData)
        model.startLoading()
        #expect(model.readTask == nil)
    }

    @Test(.timeLimit(.minutes(1)))
    func aLateRecoveryReadCannotReviveACancelledPayer() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let request = try BsvMultisigFixtures.paymentRequest()
        let saved = BsvMultisigPublicFundingFixtures.result(request: request, payer: fixture.wallet)
        let service = BsvMultisigPublicFundingServiceMock()
        let gate = BsvMultisigAsyncGate()
        service.onResults = { await gate.wait(); return [saved] }
        let model = makeModel(service: service, payer: fixture.wallet)
        model.startLoading()
        let read = try #require(model.readTask)
        await gate.started()
        model.cancel()
        await gate.release()
        await read.value
        #expect(model.recoveryState.isNoData)
        #expect(model.result == nil)
        #expect(service.calls == ["results", "cancel"])
    }

    @Test
    func anUnsignedReadFailureDoesNotPublishAPartialOrEmptySnapshot() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let request = try BsvMultisigFixtures.paymentRequest()
        let saved = BsvMultisigPublicFundingFixtures.result(request: request, payer: fixture.wallet)
        let service = BsvMultisigPublicFundingServiceMock()
        service.onResults = { [saved] }
        service.onUnsubmitted = { throw ReadFailure() }
        let model = makeModel(service: service, payer: fixture.wallet, entry: .payment(.paymentRequest(request: request)))
        try await load(model)
        #expect(model.recoveryState.isError)
        #expect(model.recoveryState.value == nil)
        #expect(model.result == nil)
        #expect(model.errorMessage == Localized.Errors.errorOccurred)
        #expect(!model.canPrepare(amount: "0.00042"))
        service.onUnsubmitted = { [] }
        model.reloadResults()
        try await (#require(model.readTask)).value
        #expect(model.recoveryState.value?.results.map(\.operationId) == [saved.operationId])
        #expect(model.errorMessage == nil)
    }

    @Test
    func preparationUsesTheLocaleAmountAndReviewAcknowledgementIsSnapshotBound() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let request = try BsvMultisigFixtures.paymentRequest()
        let prepared = BsvMultisigPublicFundingFixtures.preparation(request: request, payer: fixture.wallet)
        let service = BsvMultisigPublicFundingServiceMock()
        service.onPrepare = { _, _ in prepared }
        var clock: TimeInterval = 0
        let model = BsvMultisigPublicFundingViewModel(entry: .payment(.paymentRequest(request: request)), payer: fixture.wallet, makeService: { service }, uptime: { clock }, locale: Locale(identifier: "de_DE"))
        try await load(model)
        var reviews: [String] = []
        let task = try #require(model.prepare(amount: "0,00042", navigation: nil) { reviews.append($0.id) })
        await task.value
        #expect(service.amounts == ["0.00042"])
        #expect(reviews == [prepared.id])
        #expect(!model.canSubmit(prepared.id))
        model.setReviewed(true, preparationId: "other-operation")
        #expect(!model.canSubmit(prepared.id))
        model.setReviewed(true, preparationId: prepared.id)
        #expect(model.canSubmit(prepared.id))
        clock = 301
        #expect(!model.canSubmit(prepared.id))
        #expect(!service.calls.contains("begin"))
    }

    @Test(.timeLimit(.minutes(1)))
    func editingDuringPreparationRetiresTheOldReviewWithoutClosingTheService() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let request = try BsvMultisigFixtures.paymentRequest()
        let old = BsvMultisigPublicFundingFixtures.preparation(id: "old-operation", request: request, payer: fixture.wallet)
        let next = BsvMultisigPublicFundingFixtures.preparation(id: "new-operation", request: request, payer: fixture.wallet)
        let service = BsvMultisigPublicFundingServiceMock()
        let gate = BsvMultisigAsyncGate()
        service.onPrepare = { _, _ in await gate.wait(); return old }
        let model = makeModel(service: service, payer: fixture.wallet, entry: .payment(.paymentRequest(request: request)))
        try await load(model)
        var reviews: [String] = []
        let first = try #require(model.prepare(amount: "0.00042", navigation: nil) { reviews.append($0.id) })
        await gate.started()
        model.cancelActions()
        await gate.release()
        await first.value
        #expect(model.preparation == nil)
        #expect(reviews.isEmpty)
        #expect(!service.calls.contains("cancel"))
        service.onPrepare = { _, _ in next }
        try await (#require(model.prepare(amount: "0.00084", navigation: nil) { reviews.append($0.id) })).value
        #expect(model.preparation?.id == next.id)
        #expect(reviews == [next.id])
        #expect(service.amounts == ["0.00042", "0.00084"])
    }

    @Test(.timeLimit(.minutes(1)))
    func removingTheFormPrefixSynchronouslyCancelsItsPendingReview() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let request = try BsvMultisigFixtures.paymentRequest()
        let prepared = BsvMultisigPublicFundingFixtures.preparation(request: request, payer: fixture.wallet)
        let service = BsvMultisigPublicFundingServiceMock()
        let gate = BsvMultisigAsyncGate()
        service.onPrepare = { _, _ in await gate.wait(); return prepared }
        let model = makeModel(service: service, payer: fixture.wallet, entry: .payment(.paymentRequest(request: request)))
        try await load(model)
        let navigation = NavigationPathState()
        navigation.append("public-form")
        var completions = 0
        let task = try #require(model.prepare(amount: "0.00042", navigation: navigation) { _ in completions += 1 })
        await gate.started()
        navigation.append("internal-page")
        navigation.removeLast()
        #expect(model.isWorking)
        navigation.reset()
        #expect(!model.isWorking)
        await gate.release()
        await task.value
        #expect(completions == 0)
        #expect(model.preparation == nil)
        #expect(!service.calls.contains("cancel"))
    }

    @Test
    func recoveryRequiresAnExactOperationSelection() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let request = try BsvMultisigFixtures.paymentRequest()
        let first = BsvMultisigPublicFundingFixtures.result(id: "first-operation", request: request, payer: fixture.wallet)
        let selected = BsvMultisigPublicFundingFixtures.result(id: "selected-operation", request: request, payer: fixture.wallet, status: .pending)
        let service = BsvMultisigPublicFundingServiceMock()
        service.onResults = { [first, selected] }
        let model = makeModel(service: service, payer: fixture.wallet)
        try await load(model)
        #expect(model.result == nil)
        #expect(!model.selectRecovery("missing-operation"))
        #expect(model.selectRecovery(selected.operationId))
        #expect(model.result?.operationId == selected.operationId)
        #expect(model.result?.status == .pending)
        #expect(!service.calls.contains("begin"))
    }

    @Test
    func discardCanOnlyTargetTheExplicitUnsignedRecord() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let request = try BsvMultisigFixtures.paymentRequest()
        let signed = BsvMultisigPublicFundingFixtures.result(request: request, payer: fixture.wallet)
        let unsigned = BsvMultisigPublicFundingFixtures.unsubmitted(request: request, payer: fixture.wallet)
        let service = BsvMultisigPublicFundingServiceMock()
        service.onResults = { [signed] }
        service.onUnsubmitted = { [unsigned] }
        service.onDiscard = { _ in }
        let model = makeModel(service: service, payer: fixture.wallet)
        try await load(model)
        model.discardUnsubmitted(signed.operationId)
        #expect(model.readTask == nil)
        #expect(service.operations.isEmpty)
        model.discardUnsubmitted(unsigned.operationId)
        try await (#require(model.readTask)).value
        #expect(service.operations == [unsigned.operationId])
        #expect(!service.calls.contains("begin"))
        #expect(model.recoveryState.value?.results.map(\.operationId) == [signed.operationId])
    }

    @Test
    func acceptedRecoveryResultSurvivesAFailedReadOnlyRefresh() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let request = try BsvMultisigFixtures.paymentRequest()
        let pending = BsvMultisigPublicFundingFixtures.result(request: request, payer: fixture.wallet, status: .pending)
        let accepted = BsvMultisigPublicFundingFixtures.result(request: request, payer: fixture.wallet)
        let service = BsvMultisigPublicFundingServiceMock()
        service.onResults = { [pending] }
        service.onSync = { _ in accepted }
        let model = makeModel(service: service, payer: fixture.wallet)
        try await load(model)
        #expect(model.selectRecovery(pending.operationId))
        service.onResults = { throw ReadFailure() }
        model.retrySync(pending.operationId)
        try await (#require(model.readTask)).value
        #expect(model.result?.operationId == accepted.operationId)
        #expect(model.result?.transactionId == accepted.transactionId)
        #expect(model.result?.status == .broadcast)
        #expect(model.syncFailed)
        #expect(model.errorMessage == Localized.Errors.errorOccurred)
        #expect(service.operations == [pending.operationId])
        #expect(!service.calls.contains("begin"))
    }

    @Test
    func directlyCancellingTheFirstReadPollAllowsAnExplicitRetry() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let service = BsvMultisigPublicFundingServiceMock()
        let model = BsvMultisigPublicFundingViewModel(entry: .recovery, payer: fixture.wallet, makeService: {
            service.recordFactoryCreation()
            return service
        })
        model.startLoading()
        let read = try #require(model.readTask)
        read.cancel()
        await read.value
        #expect(service.factoryCreationCount == 0)
        #expect(model.readTask == nil)
        #expect(model.recoveryState.isNoData)
        try await load(model)
        #expect(service.factoryCreationCount == 1)
        #expect(model.recoveryState.value != nil)
        #expect(!service.calls.contains("cancel"))
    }

    @Test(arguments: [false, true])
    func cancellingThePayerOwnerPreservesTheBorrowedOriginalPaymailPreparation(permanent: Bool) async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let service = BsvMultisigPublicFundingServiceMock()
        let model = makeModel(service: service, payer: fixture.wallet, entry: .payment(.paymail(service: fixture.service, preparation: preparation)))
        try await load(model)
        if permanent {
            model.cancel()
        } else {
            model.cancelActions()
        }
        let final = try await fixture.service.confirmPublicFunding(preparation: preparation, value: 84000)
        #expect(final.amount == "0.00084")
        #expect(final.fingerprint == preparation.paymentRequest().fingerprint)
        #expect(service.calls.contains("cancel") == permanent)
        #expect(fixture.credentialAccessCount == 0)
        #expect(fixture.rejectedRequestCount == 0)
    }

    private func makeModel(service: BsvMultisigPublicFundingServiceMock, payer: Wallet, entry: BsvMultisigPublicFundingEntry = .recovery) -> BsvMultisigPublicFundingViewModel {
        BsvMultisigPublicFundingViewModel(entry: entry, payer: payer, makeService: { service }, locale: Locale(identifier: "en_US"))
    }

    private func load(_ model: BsvMultisigPublicFundingViewModel) async throws {
        model.startLoading()
        try await (#require(model.readTask)).value
    }

    private struct ReadFailure: LocalizedError {
        var errorDescription: String? { "Untrusted public fixture response" }
    }
}
