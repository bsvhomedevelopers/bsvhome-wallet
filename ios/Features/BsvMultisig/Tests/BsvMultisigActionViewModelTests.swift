@testable import BsvMultisig
import Components
import Foundation
import BsvHomeCore
import BsvHomeCorePrimitivesTestKit
import Localization
import SwiftUI
import Testing

@MainActor
struct BsvMultisigActionViewModelTests {
    enum Mutation: CaseIterable, Sendable {
        case prepare, sign, broadcast, funding, retryFunding, withdraw, paymail
    }

    @Test(arguments: [false, true])
    func anInvalidRecipientScanReportsAnErrorWithoutCallingTheFormUpdater(hasExistingInput: Bool) {
        let service = BsvMultisigServiceMock()
        let model = BsvMultisigViewModel(makeService: { service })
        let originalDestination = hasExistingInput ? "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5" : ""
        let originalAmount = hasExistingInput ? "0.00042" : ""
        var destination = originalDestination
        var amount = originalAmount
        var updates = 0
        model.scanRecipient("invalid-public-code") { payment in
            updates += 1
            destination = payment.recipient.address
            if let value = payment.amount {
                amount = value
            }
        }
        #expect(updates == 0)
        #expect(destination == originalDestination)
        #expect(amount == originalAmount)
        #expect(model.errorMessage == Localized.Errors.invalidAddressName)
        #expect(service.calls.isEmpty)
    }

    @Test(arguments: [false, true])
    func aValidRecipientScanOnlyReplacesAnAmountPresentInTheValidatedRequest(hasAmount: Bool) {
        let service = BsvMultisigServiceMock()
        let model = BsvMultisigViewModel(makeService: { service })
        let address = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5"
        let code = hasAmount ? "bitcoinsv:\(address)?amount=0.00000001" : address
        var destination = "previous-public-address"
        var amount = "0.00042"
        var updates = 0
        model.errorMessage = Localized.Errors.invalidAddressName
        model.scanRecipient(code) { payment in
            updates += 1
            destination = payment.recipient.address
            if let value = payment.amount {
                amount = value
            }
        }
        #expect(updates == 1)
        #expect(destination == address)
        #expect(amount == (hasAmount ? "0.00000001" : "0.00042"))
        #expect(model.errorMessage == nil)
        #expect(service.calls.isEmpty)
    }

    @Test
    func anInvalidRecipientScanCancelsTheOldScopeBeforeItsMutationPolls() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service })
        let task = try #require(model.sign(BsvMultisigFixtures.review()))
        let scope = try #require(service.actionScopes.last)
        var updates = 0
        model.scanRecipient("invalid-public-code") { _ in updates += 1 }
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try scope.check() }
        #expect(!model.isWorking)
        #expect(await task.value == nil)
        #expect(updates == 0)
        #expect(!service.calls.contains { $0.hasPrefix("sign:") })
        #expect(model.errorMessage == Localized.Errors.invalidAddressName)
    }

    @Test
    func aRetiredModelCannotDeliverALateValidatedRecipient() {
        let service = BsvMultisigServiceMock()
        let model = BsvMultisigViewModel(makeService: { service })
        model.cancel()
        var updates = 0
        model.scanRecipient("1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5") { _ in updates += 1 }
        #expect(updates == 0)
    }

    @Test(arguments: Mutation.allCases)
    func cancellingBeforeFirstPollPreventsEveryMutation(mutation: Mutation) async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        var cancellations: [String] = []
        let model = BsvMultisigViewModel(makeService: { service }, cancelAuthorization: { cancellations.append($0) })
        await model.load()
        await model.loadPaymail("multisig")
        let calls = service.calls
        let waiting = try start(mutation, model: model)
        let scope = try #require(service.actionScopes.last)
        #expect(service.actionIntents.count == 1)
        #expect(model.isWorking)
        model.cancelActions()
        #expect(!model.isWorking)
        #expect(cancellations == [scope.id()])
        #expect(try scope.outcome().submission == .cancelledBeforeSubmission)
        await waiting()
        #expect(service.calls == calls)
        #expect(!service.calls.contains("cancel"))
        #expect(await model.retryFunding("multisig")?.value != nil)
    }

    @Test
    func routeRemovalCancelsSynchronouslyWhileInternalNavigationPreservesTheAction() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let owner = NavigationPathState()
        owner.binding.wrappedValue.append(BsvMultisigRoute.wallet("multisig"))
        owner.binding.wrappedValue.append(BsvMultisigRoute.review("proposal", purpose: .sign))
        var cancellations = 0
        let model = BsvMultisigViewModel(makeService: { service }, cancelAuthorization: { _ in cancellations += 1 })
        let task = try #require(model.sign(BsvMultisigFixtures.review(), navigation: owner))
        let scope = try #require(service.actionScopes.last)
        owner.binding.wrappedValue.append(BsvMultisigRoute.settings("multisig"))
        owner.removeLast()
        #expect(cancellations == 0)
        #expect(throws: Never.self) { try scope.check() }
        owner.binding.wrappedValue = NavigationPath([BsvMultisigRoute.wallet("multisig"), BsvMultisigRoute.settings("multisig")])
        #expect(cancellations == 1)
        #expect(!model.isWorking)
        #expect(try scope.outcome().submission == .cancelledBeforeSubmission)
        #expect(await task.value == nil)
        #expect(!service.calls.contains { $0.hasPrefix("sign:") })
        owner.reset()
        #expect(cancellations == 1)
    }

    @Test(arguments: [false, true])
    func cancellingTheOwnedSwiftTaskClosesItsScopeAndPresentationOnce(afterStarting: Bool) async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let gate = BsvMultisigAsyncGate()
        service.onAction = { _ in await gate.wait() }
        let closed = AsyncStream<Void>.makeStream()
        var cancellations = 0
        let model = BsvMultisigViewModel(makeService: { service }, cancelAuthorization: { _ in
            cancellations += 1
            closed.continuation.yield(())
        })
        let task = try #require(model.sign(BsvMultisigFixtures.review()))
        let scope = try #require(service.actionScopes.last)
        if afterStarting {
            await gate.started()
        }
        task.cancel()
        var signals = closed.stream.makeAsyncIterator()
        try #require(await signals.next() != nil)
        #expect(!model.isWorking)
        #expect(try scope.outcome().submission == .cancelledBeforeSubmission)
        if afterStarting {
            await gate.release()
        }
        #expect(await task.value == nil)
        #expect(cancellations == 1)
    }

    @Test
    func aLateCancelledResponseCannotCompleteOrResetItsReplacement() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let gate = BsvMultisigAsyncGate()
        service.onAction = { _ in await gate.wait() }
        var oldCompletions = 0
        var nextCompletions = 0
        let model = BsvMultisigViewModel(makeService: { service })
        let first = try #require(model.sign(BsvMultisigFixtures.review()) { _ in oldCompletions += 1 })
        await gate.started()
        model.cancelActions()
        service.onAction = nil
        let next = try #require(model.broadcast(BsvMultisigFixtures.review()) { _ in nextCompletions += 1 })
        #expect(await next.value != nil)
        await gate.release()
        #expect(await first.value == nil)
        #expect(oldCompletions == 0)
        #expect(nextCompletions == 1)
        #expect(!model.isWorking)
        #expect(model.errorMessage == nil)
    }

    @Test
    func aReturnedMutationSurvivesAProjectionReloadFailureWithoutReplay() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        service.onAction = { _ in service.snapshotError = BsvHomeBsvMultisigError.Storage(reason: "fixture failure") }
        var completedProposal: String?
        let model = BsvMultisigViewModel(makeService: { service })
        let task = try #require(model.sign(BsvMultisigFixtures.review()) { completedProposal = $0.id })
        let proposal = await task.value
        #expect(proposal?.id == "proposal")
        #expect(completedProposal == "proposal")
        #expect(model.actionSyncFailed)
        #expect(model.errorMessage == Localized.multisigErrorStorage)
        #expect(service.calls.filter { $0.hasPrefix("sign:") }.count == 1)
        service.snapshotError = nil
        await model.refresh()
        #expect(service.calls.filter { $0.hasPrefix("sign:") }.count == 1)
    }

    @Test
    func signingUsesTheFrozenOpaqueReviewWithoutReplacingItsVersionOrContent() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service })
        let review = BsvMultisigFixtures.review()
        var changed = review.proposal
        changed.destination = "changed-public-destination"
        service.proposalRecords = [changed]
        await model.load()
        #expect(!review.canSubmit(purpose: .sign, currentProposal: model.proposal("proposal"), hasReviewed: true))
        #expect(await model.sign(review)?.value != nil)
        #expect(service.actionIntents == [.signProposal(proposalId: review.proposal.id, expectedVersion: review.proposal.version, reviewId: review.reviewId)])
    }

    @Test
    func aCancelledPreparationCannotPublishOrNavigateALateReview() async throws {
        let service = BsvMultisigServiceMock()
        let gate = BsvMultisigAsyncGate()
        service.onPrepareFunding = { await gate.wait(); return BsvMultisigFixtures.preparation() }
        let owner = NavigationPathState()
        owner.binding.wrappedValue.append(BsvMultisigRoute.funding("multisig"))
        var reviews = 0
        let model = BsvMultisigViewModel(makeService: { service })
        let task = try #require(model.prepareFunding(walletId: "multisig", payerWalletId: "payer", amount: "0.00042", navigation: owner) { _ in reviews += 1 })
        await gate.started()
        owner.reset()
        await gate.release()
        #expect(await task.value == nil)
        #expect(model.fundingPreparation == nil)
        #expect(reviews == 0)
        #expect(!service.calls.contains("cancel"))
    }

    @Test
    func aPaymailConfirmationUsesTheCoreCanonicalAlias() {
        let model = BsvMultisigViewModel(makeService: { BsvMultisigServiceMock() })
        #expect(model.canonicalPaymailAlias("  Team-Name  ") == "team-name")
        #expect(model.canonicalPaymailAlias("not@an-alias") == nil)
        #expect(model.errorMessage == Localized.multisigPaymailInvalidName)
    }

    @Test
    func theOpaqueScopeFixtureDoesNotUseCredentialsOrNetwork() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let first = try fixture.beginRetryFunding()
        first.cancel()
        #expect(try first.outcome().submission == .cancelledBeforeSubmission)
        let next = try fixture.beginRetryFunding()
        #expect(throws: Never.self) { try next.check() }
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    private func start(_ mutation: Mutation, model: BsvMultisigViewModel) throws -> @MainActor () async -> Void {
        let review = BsvMultisigFixtures.review()
        switch mutation {
        case .prepare:
            let task = try #require(model.createProposal(walletId: "multisig", destination: "1BitcoinEaterAddressDontSendf59kuE", amount: "0.00042", memo: ""))
            return { _ = await task.value }
        case .sign:
            let task = try #require(model.sign(review))
            return { _ = await task.value }
        case .broadcast:
            let task = try #require(model.broadcast(review))
            return { _ = await task.value }
        case .withdraw:
            let task = try #require(model.withdraw(review))
            return { _ = await task.value }
        case .funding:
            let task = try #require(model.submitFunding("review"))
            return { _ = await task.value }
        case .retryFunding:
            let task = try #require(model.retryFunding("multisig"))
            return { _ = await task.value }
        case .paymail:
            let settings = try #require(model.paymailSettings["multisig"])
            let task = try #require(model.updatePaymail(settings, alias: "changed", active: false))
            return { _ = await task.value }
        }
    }
}
