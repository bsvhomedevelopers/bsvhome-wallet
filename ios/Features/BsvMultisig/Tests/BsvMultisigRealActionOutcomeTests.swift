import BsvHomeCorePrimitivesTestKit
@testable import BsvMultisig
import Foundation
import BsvHomeCore
import Localization
import Testing

@MainActor
struct BsvMultisigRealActionOutcomeTests {
    @Test(.timeLimit(.minutes(1)))
    func aRealPreparePostFailureRemainsUnknownAndDoesNotReplayOnRefresh() async throws {
        let fixture = try await BsvMultisigActionOutcomeFixture.make(scenario: .unknownAfterPost)
        defer { fixture.cancel() }
        let model = BsvMultisigViewModel(makeService: { fixture.service }, locale: Locale(identifier: "en_US_POSIX"))
        await model.load()
        #expect(model.errorMessage == nil)
        #expect(model.wallets.first?.canTransact == true)
        var completions: [String] = []
        let task = try #require(model.createProposal(walletId: BsvMultisigActionOutcomeFixture.walletId, destination: fixture.destination, amount: fixture.amount, memo: fixture.memo) {
            completions.append($0)
        })
        #expect(await task.value == nil)
        let scope = try #require(fixture.lastScope)
        #expect(try scope.outcome().submission == .unknown)
        #expect(try scope.outcome().syncFailed == false)
        #expect(model.actionOutcome?.submission == .unknown)
        #expect(model.errorMessage == Localized.multisigUncertain)
        #expect(!model.actionSyncFailed)
        #expect(completions.isEmpty)
        #expect(fixture.businessPostCount == 1)
        #expect(fixture.successfulStateWriteCount == 1)
        #expect(fixture.authorizationCount == 1)
        #expect(fixture.authenticationCount == 1)
        #expect(fixture.unexpectedRequestCount == 0)
        #expect(fixture.forbiddenCredentialCount == 0)
        await model.refresh()
        #expect(fixture.businessPostCount == 1)
        #expect(try scope.outcome().submission == .unknown)
        #expect(model.actionOutcome?.submission == .unknown)
        #expect(completions.isEmpty)
        #expect(!model.isWorking)
    }

    @Test(.timeLimit(.minutes(1)))
    func aRealSubmittedPrepareSurvivesNativeProjectionFailureAndAnExplicitRefreshRecovers() async throws {
        let fixture = try await BsvMultisigActionOutcomeFixture.make(scenario: .submittedThenProjectionUnavailable)
        defer { fixture.cancel() }
        let model = BsvMultisigViewModel(makeService: { fixture.service }, locale: Locale(identifier: "en_US_POSIX"))
        await model.load()
        #expect(model.errorMessage == nil)
        var completions: [String] = []
        let task = try #require(model.createProposal(walletId: BsvMultisigActionOutcomeFixture.walletId, destination: fixture.destination, amount: fixture.amount, memo: fixture.memo) {
            completions.append($0)
        })
        let returnedId = await task.value
        #expect(returnedId == BsvMultisigActionOutcomeFixture.proposalId)
        #expect(completions == [BsvMultisigActionOutcomeFixture.proposalId])
        let scope = try #require(fixture.lastScope)
        let outcome = try scope.outcome()
        #expect(outcome.submission == .submitted)
        #expect(outcome.proposalId == returnedId)
        #expect(!outcome.syncFailed)
        #expect(model.actionOutcome?.submission == .submitted)
        #expect(model.actionOutcome?.proposalId == returnedId)
        #expect(model.actionSyncFailed)
        #expect(model.errorMessage == Localized.multisigErrorStorage)
        #expect(fixture.projectionReadFaultCount == 1)
        #expect(fixture.successfulStateWriteCount == 2)
        #expect(fixture.businessPostCount == 1)
        #expect(fixture.authorizationCount == 1)
        #expect(fixture.authenticationCount == 1)
        #expect(fixture.unexpectedRequestCount == 0)
        #expect(fixture.forbiddenCredentialCount == 0)
        let proposal = try fixture.service.getProposal(proposalId: #require(returnedId))
        #expect(proposal.id == BsvMultisigActionOutcomeFixture.proposalId)
        #expect(proposal.destination == fixture.destination)
        #expect(proposal.valueSatoshis == "42000")
        #expect(proposal.feeSatoshis == "200")
        #expect(proposal.memo == fixture.memo)
        await model.refresh()
        #expect(model.errorMessage == nil)
        #expect(model.proposals.map(\.id) == [BsvMultisigActionOutcomeFixture.proposalId])
        #expect(!model.actionSyncFailed)
        #expect(model.actionOutcome?.submission == .submitted)
        #expect(model.actionOutcome?.proposalId == returnedId)
        #expect(try scope.outcome() == outcome)
        #expect(fixture.projectionReadFaultCount == 1)
        #expect(fixture.businessPostCount == 1)
        #expect(completions == [BsvMultisigActionOutcomeFixture.proposalId])
        #expect(!model.isWorking)
    }

    @Test(.timeLimit(.minutes(1)))
    func aRealCoreCleanupFailureRetainsItsRecoveryWarningAfterAReadOnlyRefresh() async throws {
        let fixture = try await BsvMultisigActionOutcomeFixture.make(scenario: .submittedThenStateCleanupUnavailable)
        defer { fixture.cancel() }
        let model = BsvMultisigViewModel(makeService: { fixture.service }, locale: Locale(identifier: "en_US_POSIX"))
        await model.load()
        #expect(model.errorMessage == nil)
        var completions: [String] = []
        let task = try #require(model.createProposal(walletId: BsvMultisigActionOutcomeFixture.walletId, destination: fixture.destination, amount: fixture.amount, memo: fixture.memo) {
            completions.append($0)
        })
        #expect(await task.value == nil)
        let scope = try #require(fixture.lastScope)
        let outcome = try scope.outcome()
        #expect(outcome.submission == .submitted)
        #expect(outcome.proposalId == BsvMultisigActionOutcomeFixture.proposalId)
        #expect(outcome.syncFailed)
        #expect(model.actionOutcome == outcome)
        #expect(model.actionSyncFailed)
        #expect(completions.isEmpty)
        #expect(fixture.cleanupWriteFaultCount == 1)
        #expect(fixture.projectionReadFaultCount == 0)
        #expect(fixture.successfulStateWriteCount == 1)
        #expect(fixture.businessPostCount == 1)
        #expect(fixture.authorizationCount == 1)
        #expect(fixture.authenticationCount == 1)
        #expect(fixture.unexpectedRequestCount == 0)
        #expect(fixture.forbiddenCredentialCount == 0)
        await model.refresh()
        #expect(model.errorMessage == nil)
        #expect(model.proposals.map(\.id) == [BsvMultisigActionOutcomeFixture.proposalId])
        #expect(model.actionSyncFailed)
        #expect(model.actionOutcome == outcome)
        #expect(try scope.outcome() == outcome)
        #expect(fixture.businessPostCount == 1)
        #expect(fixture.cleanupWriteFaultCount == 1)
        #expect(completions.isEmpty)
    }
}
