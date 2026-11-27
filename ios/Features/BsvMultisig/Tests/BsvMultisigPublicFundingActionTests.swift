@testable import BsvMultisig
import Components
import BsvHomeCore
import BsvHomeCorePrimitivesTestKit
import Testing

@MainActor
struct BsvMultisigPublicFundingActionTests {
    @Test(arguments: [false, true])
    func cancellationBeforeTheSubmissionPollClosesTheRealScopeWithoutSubmitting(permanent: Bool) async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let request = try BsvMultisigFixtures.paymentRequest()
        let prepared = BsvMultisigPublicFundingFixtures.preparation(request: request, payer: fixture.payer)
        let service = BsvMultisigPublicFundingServiceMock()
        service.actionScopeFactory = { try fixture.beginRetryFunding() }
        service.onPrepare = { _, _ in prepared }
        let model = BsvMultisigPublicFundingViewModel(entry: .payment(.paymentRequest(request: request)), payer: fixture.payer, makeService: { service })
        model.startLoading()
        try await (#require(model.readTask)).value
        try await (#require(model.prepare(amount: "0.00042", navigation: nil) { _ in })).value
        model.setReviewed(true, preparationId: prepared.id)
        var completions = 0
        let task = try #require(model.submit(prepared.id, navigation: nil) { _ in completions += 1 })
        let scope = try #require(service.scopes.last)
        try scope.check()
        if permanent {
            model.cancel()
        } else {
            model.cancelActions()
        }
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try scope.check() }
        await task.value
        #expect(!service.calls.contains("submit"))
        #expect(service.calls.contains("cancel") == permanent)
        #expect(try scope.outcome().submission == .cancelledBeforeSubmission)
        #expect(model.outcome?.submission == .cancelledBeforeSubmission)
        #expect(model.result == nil)
        #expect(!model.isWorking)
        #expect(completions == 0)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test(.timeLimit(.minutes(1)))
    func trueNavigationExitCancelsTheScopeAndRejectsALateSubmissionResult() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let request = try BsvMultisigFixtures.paymentRequest()
        let prepared = BsvMultisigPublicFundingFixtures.preparation(request: request, payer: fixture.payer)
        let savedRecord = try await fixture.service.fundingResult(operationId: fixture.operationId)
        let saved = try #require(savedRecord)
        let service = BsvMultisigPublicFundingServiceMock()
        service.actionScopeFactory = { try fixture.beginRetryFunding() }
        service.onPrepare = { _, _ in prepared }
        let gate = BsvMultisigAsyncGate()
        service.onSubmit = { _, _ in await gate.wait(); return saved }
        let model = BsvMultisigPublicFundingViewModel(entry: .payment(.paymentRequest(request: request)), payer: fixture.payer, makeService: { service })
        model.startLoading()
        try await (#require(model.readTask)).value
        try await (#require(model.prepare(amount: "0.00042", navigation: nil) { _ in })).value
        model.setReviewed(true, preparationId: prepared.id)
        let navigation = NavigationPathState()
        navigation.binding.wrappedValue.append(BsvMultisigPublicFundingRoute.review(prepared.id))
        var completions = 0
        let task = try #require(model.submit(prepared.id, navigation: navigation) { _ in completions += 1 })
        await gate.started()
        let scope = try #require(service.scopes.last)
        navigation.binding.wrappedValue.append("internal-page")
        navigation.removeLast()
        try scope.check()
        #expect(model.isWorking)
        navigation.reset()
        #expect(!model.isWorking)
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try scope.check() }
        await gate.release()
        await task.value
        #expect(model.result == nil)
        #expect(model.outcome?.submission == .cancelledBeforeSubmission)
        #expect(completions == 0)
        #expect(!service.calls.contains("cancel"))
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }
}
