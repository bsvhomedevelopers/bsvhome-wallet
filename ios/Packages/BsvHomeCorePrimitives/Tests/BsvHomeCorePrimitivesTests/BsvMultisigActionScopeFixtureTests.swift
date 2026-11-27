import BsvHomeCore
import BsvHomeCorePrimitivesTestKit
import Testing

struct BsvMultisigActionScopeFixtureTests {
    @Test
    func aVerifiedPublicJournalCreatesACancellableScopeWithoutAuthorizationOrSigning() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let record = try await fixture.service.fundingResult(operationId: fixture.operationId)
        let saved = try #require(record)
        #expect(saved.operationId == fixture.operationId)
        #expect(saved.payerWalletId == fixture.payerWalletId)
        #expect(saved.status == .pending)
        let scope = try fixture.beginRetryFunding()
        try scope.check()
        #expect(try scope.outcome().submission == .notSubmitted)
        scope.cancel()
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try scope.check() }
        #expect(try scope.outcome().submission == .cancelledBeforeSubmission)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }
}
