@testable import BsvMultisig
import Foundation
import BsvHomeCore
import Localization
import Observation
import SwiftUI
import Testing

@MainActor
struct BsvMultisigViewModelTests {
    @Test
    func publishesCoreCapabilityWhenFirstLoadHasNoRecords() async {
        let service = BsvMultisigServiceMock(recordsAreEmpty: true)
        let model = BsvMultisigViewModel(makeService: { service })
        let changes = AsyncStream<Bool>.makeStream()
        withObservationTracking {
            #expect(!model.supportsAsset("bitcoinsv"))
        } onChange: { changes.continuation.yield(true) }
        await model.load()
        changes.continuation.finish()
        var iterator = changes.stream.makeAsyncIterator()
        #expect(await iterator.next() == true)
        #expect(model.wallets.isEmpty)
        #expect(model.proposals.isEmpty)
        #expect(model.supportsAsset("bitcoinsv"))
        #expect(!model.supportsAsset("ethereum"))
    }

    @Test
    func preservesReviewedVersionAndReloadsConflictWithoutResigning() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service })
        await model.load()
        service.operationError = BsvHomeBsvMultisigError.ConcurrentUpdate
        #expect(await model.sign(BsvMultisigFixtures.review())?.value == nil)
        #expect(service.calls.filter { $0.hasPrefix("sign:") } == ["sign:7"])
        #expect(service.calls.filter { $0 == "load" }.count == 2)
        #expect(model.errorMessage == Localized.multisigErrorConcurrentUpdate)
    }

    @Test
    func cancellationDropsLateResponseAndDoesNotShowFailure() async {
        let gate = BsvMultisigAsyncGate()
        let service = BsvMultisigServiceMock(loadAction: { await gate.wait() })
        let model = BsvMultisigViewModel(makeService: { service })
        let loading = Task { await model.load() }
        await gate.started()
        model.cancel()
        await gate.release()
        await loading.value
        #expect(model.wallets.isEmpty)
        #expect(model.pendingCount == 0)
        #expect(model.errorMessage == nil)
        #expect(service.calls.contains("cancel"))
    }

    @Test
    func fullLoadPreventsOverlappingPageRefreshAndSigning() async {
        let gate = BsvMultisigAsyncGate()
        let service = BsvMultisigServiceMock(loadAction: { await gate.wait() })
        let model = BsvMultisigViewModel(makeService: { service })
        let loading = Task { await model.load() }
        await gate.started()
        await model.refresh()
        await model.refreshWallet("multisig")
        #expect(await model.sign(BsvMultisigFixtures.review())?.value == nil)
        #expect(service.calls == ["load"])
        await gate.release()
        await loading.value
        await model.refreshWallet("multisig")
        #expect(service.calls == ["load", "refresh-wallet"])
    }

    @Test
    func creationFreezesIdentityAndSupportsFiveMemberReview() async {
        let service = BsvMultisigServiceMock()
        let model = BsvMultisigViewModel(makeService: { service })
        let review = await model.beginCreation(name: "Team", threshold: 3, memberNames: ["A", "B", "C", "D", "E"], signerWalletId: "reviewed-member")
        #expect(review?.identity.walletId == "reviewed-member")
        #expect(service.calls == ["begin:reviewed-member:3:5"])
        if let review {
            #expect(await model.submitCreation(review) == "multisig")
        }
        #expect(service.calls == ["begin:reviewed-member:3:5", "submit:creation-draft"])
    }

    @Test
    func uncertainCreationRequiresExplicitCandidateSelectionWithoutResubmitting() async throws {
        let service = BsvMultisigServiceMock()
        service.creationSubmitError = BsvHomeBsvMultisigError.CreationUncertain(draftId: "creation-draft")
        service.creationCandidates = [BsvMultisigFixtures.wallet(id: "candidate-a"), BsvMultisigFixtures.wallet(id: "candidate-b")]
        let model = BsvMultisigViewModel(makeService: { service })
        let review = try #require(await model.beginCreation(name: "Team", threshold: 2, memberNames: ["A", "B"], signerWalletId: "member"))
        #expect(await model.submitCreation(review) == nil)
        #expect(model.uncertainCreationDraftId == review.draft.id)
        #expect(model.creationCandidates == nil)
        #expect(service.calls == ["begin:member:2:2", "submit:creation-draft"])

        await model.reconcileCreation(review)
        #expect(model.creationCandidates?.map(\.id) == ["candidate-a", "candidate-b"])
        #expect(service.calls == ["begin:member:2:2", "submit:creation-draft", "reconcile:creation-draft"])
        #expect(await model.acceptCreation(review, walletId: "candidate-b") == "candidate-b")
        #expect(service.calls == ["begin:member:2:2", "submit:creation-draft", "reconcile:creation-draft", "accept:creation-draft:candidate-b"])
    }

    @Test
    func aSubmittedDraftOffersReconciliationWithoutAnotherSubmission() async throws {
        let service = BsvMultisigServiceMock()
        service.creationStatus = .submitted
        let model = BsvMultisigViewModel(makeService: { service })
        let review = try #require(await model.beginCreation(name: "Team", threshold: 2, memberNames: ["A", "B"], signerWalletId: "member"))
        #expect(review.draft.status == .submitted)
        await model.reconcileCreation(review)
        #expect(model.creationCandidates?.isEmpty == true)
        #expect(service.calls == ["begin:member:2:2", "reconcile:creation-draft"])
    }

    @Test
    func cancellingBeforeSubmissionStartsDoesNotCallCore() async throws {
        let service = BsvMultisigServiceMock()
        let model = BsvMultisigViewModel(makeService: { service })
        let review = try #require(await model.beginCreation(name: "Team", threshold: 2, memberNames: ["A", "B"], signerWalletId: "member"))
        let submission = Task { await model.submitCreation(review) }
        submission.cancel()
        #expect(await submission.value == nil)
        #expect(service.calls == ["begin:member:2:2"])
    }

    @Test(arguments: [false, true])
    func cancellingReconciliationDiscardsLateCandidates(swiftTaskOnly: Bool) async throws {
        let service = BsvMultisigServiceMock()
        let gate = BsvMultisigAsyncGate()
        service.onReconcileCreation = { await gate.wait(); return [BsvMultisigFixtures.wallet()] }
        let model = BsvMultisigViewModel(makeService: { service })
        let review = try #require(await model.beginCreation(name: "Team", threshold: 2, memberNames: ["A", "B"], signerWalletId: "member"))
        let query = Task { await model.reconcileCreation(review) }
        await gate.started()
        if swiftTaskOnly {
            query.cancel()
        } else {
            model.cancel()
        }
        await gate.release()
        await query.value
        #expect(model.creationCandidates == nil)
        #expect(!service.calls.contains { $0.hasPrefix("submit:") || $0.hasPrefix("accept:") })
    }

    @Test(arguments: [false, true])
    func invitationPreviewPassesTheCoreNormalizedToken(uri: Bool) async {
        let token = String(repeating: "A", count: 43)
        let value = uri ? "bsvhome://multisig/join?token=\(token)" : token
        let service = BsvMultisigServiceMock()
        let model = BsvMultisigViewModel(makeService: { service })
        let preview = await model.previewInvitation(value)
        #expect(preview?.token == token)
        #expect(service.calls == ["preview:\(token)"])
        #expect(model.invitationQrPayload(token) == "bsvhome://multisig/join?token=\(token)")
    }

    @Test
    func invalidInvitationDoesNotConstructAnAuthenticatedService() async {
        let model = BsvMultisigViewModel(makeService: {
            Issue.record("Invalid invitation constructed an authenticated service")
            return BsvMultisigServiceMock()
        })
        #expect(await model.previewInvitation("bsvhome://multisig/join?token=invalid&other=value") == nil)
        #expect(model.errorMessage == Localized.multisigInvalidInvite)
    }

    @Test
    func proposalCreationPassesExactAmountWithoutSigning() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service })
        #expect(await model.createProposal(walletId: "multisig", destination: "recipient", amount: "0.00000001", memo: "Purpose")?.value == "proposal")
        #expect(service.calls == ["prepare:0.00000001"])
    }

    @Test(arguments: ["fr_FR", "de_DE"])
    func encodesLocaleDecimalWithoutLosingDigits(localeIdentifier: String) async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service }, locale: Locale(identifier: localeIdentifier))
        #expect(await model.createProposal(walletId: "multisig", destination: "recipient", amount: "0,001", memo: "")?.value == "proposal")
        #expect(await model.prepareFunding(walletId: "multisig", payerWalletId: "payer", amount: "0,12345678")?.value != nil)
        #expect(service.calls == ["prepare:0.001", "prepare-funding:payer:0.12345678"])
    }

    @Test(arguments: [("de_DE", "1.000,5"), ("fr_FR", "1\u{202F}000,5"), ("fr_FR", "0,12.3")])
    func rejectsGroupingAndMixedSeparatorsBeforeRequest(localeIdentifier: String, text: String) async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service }, locale: Locale(identifier: localeIdentifier))
        #expect(await model.createProposal(walletId: "multisig", destination: "recipient", amount: text, memo: "")?.value == nil)
        #expect(await model.prepareFunding(walletId: "multisig", payerWalletId: "payer", amount: text)?.value == nil)
        #expect(service.calls.isEmpty)
        #expect(model.errorMessage == Localized.Errors.invalidAmount)
    }

    @Test
    func reviewRequiresCheckboxCurrentVersionAndCorePermission() {
        let signable = BsvHomeBsvMultisigProposalReview(reviewId: "review-fixture", proposal: BsvMultisigFixtures.proposal(), fields: [])
        #expect(signable.canSubmit(purpose: .sign, currentProposal: BsvMultisigFixtures.proposal(), hasReviewed: true))
        #expect(!signable.canSubmit(purpose: .sign, currentProposal: BsvMultisigFixtures.proposal(version: 8), hasReviewed: true))
        #expect(!signable.canSubmit(purpose: .sign, currentProposal: BsvMultisigFixtures.proposal(), hasReviewed: false))
        #expect(!signable.canSubmit(purpose: .sign, currentProposal: BsvMultisigFixtures.proposal(canSign: false), hasReviewed: true))
        #expect(!signable.canSubmit(purpose: .broadcast, currentProposal: BsvMultisigFixtures.proposal(), hasReviewed: true))
        let ready = BsvHomeBsvMultisigProposalReview(reviewId: "review-fixture", proposal: BsvMultisigFixtures.proposal(canSign: false, canBroadcast: true), fields: [])
        #expect(ready.canSubmit(purpose: .broadcast, currentProposal: BsvMultisigFixtures.proposal(canSign: false, canBroadcast: true), hasReviewed: true))
        #expect(!ready.canSubmit(purpose: .sign, currentProposal: BsvMultisigFixtures.proposal(), hasReviewed: true))
    }

    @Test
    func fundingReviewUsesMonotonicFiveMinuteLifetime() async {
        var uptime: TimeInterval = 100
        let service = BsvMultisigServiceMock()
        let model = BsvMultisigViewModel(makeService: { service }, uptime: { uptime })
        #expect(await model.prepareFunding(walletId: "multisig", payerWalletId: "payer", amount: "0.00042")?.value != nil)
        #expect(model.fundingRemainingSeconds == 300)
        uptime = 399
        #expect(model.fundingRemainingSeconds == 1)
        uptime = 400
        #expect(model.fundingRemainingSeconds == 0)
    }

    @Test
    func failedBalanceRefreshKeepsCachedValuesWithVisibleError() async {
        let service = BsvMultisigServiceMock(loadAction: { throw BsvHomeBsvMultisigError.Provider(reason: "offline") })
        let model = BsvMultisigViewModel(makeService: { service })
        await model.refresh()
        #expect(model.wallet("multisig")?.balanceSatoshis == "100000")
        #expect(model.errorMessage == Localized.multisigErrorProvider)
    }

    @Test
    func pendingFundingRestoresOriginalTransactionWithoutPreparingOrSigning() async {
        let service = BsvMultisigServiceMock(pendingResult: BsvMultisigFixtures.result(status: .pending))
        let model = BsvMultisigViewModel(makeService: { service })
        #expect(await model.loadFunding("multisig"))
        #expect(model.fundingResult?.transactionId == "frozen-transaction-id")
        #expect(model.fundingResult?.status == .pending)
        #expect(service.calls.isEmpty)
    }

    @Test
    func broadcastSurvivesPayerSyncFailureAndOnlyRetriesSynchronization() async throws {
        var synchronizationCount = 0
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service }, syncPayer: { payerId in
            #expect(payerId == "payer")
            synchronizationCount += 1
            if synchronizationCount == 1 {
                throw BsvHomeBsvMultisigError.Provider(reason: "offline")
            }
        })
        #expect(await model.submitFunding("review")?.value != nil)
        #expect(model.fundingResult?.status == .broadcast)
        #expect(model.fundingResult?.transactionId == "frozen-transaction-id")
        #expect(model.fundingSyncFailed)
        await model.retryFundingSync()
        #expect(!model.fundingSyncFailed)
        #expect(service.calls == ["submit-funding:review", "sync-funding"])
        #expect(synchronizationCount == 2)
    }

    @Test
    func failedOrCancelledFundingSubmissionInvalidatesUnsignedReview() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service })
        #expect(await model.prepareFunding(walletId: "multisig", payerWalletId: "payer", amount: "0.00042")?.value != nil)
        service.operationError = BsvHomeBsvMultisigError.Cancelled
        #expect(await model.submitFunding("review")?.value == nil)
        #expect(model.fundingPreparation == nil)
        #expect(model.fundingRemainingSeconds == 0)
        #expect(model.errorMessage == nil)
        #expect(!service.calls.contains("retry-funding"))
    }

    @Test
    func fundingResultReplacesReviewRouteWithoutDuplicatingStack() {
        var path = NavigationPath()
        var publishedCounts: [Int] = []
        let navigation = BsvMultisigNavigation(path: Binding(get: { path }, set: {
            path = $0
            publishedCounts.append($0.count)
        }))
        navigation.append(.wallet("multisig"))
        navigation.append(.funding("multisig"))
        navigation.append(.fundingReview("review"))
        publishedCounts.removeAll()
        navigation.replaceLast(.fundingResult("multisig"))
        #expect(path.count == 3)
        #expect(publishedCounts == [3])
    }

    @Test
    func fundingFormReloadPreservesAcceptedResultAndSyncFailure() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service }, syncPayer: { _ in throw BsvHomeBsvMultisigError.Provider(reason: "offline") })
        #expect(await model.submitFunding("review")?.value != nil)
        #expect(await model.loadFunding("multisig"))
        #expect(model.fundingResult?.status == .broadcast)
        #expect(model.fundingResult?.transactionId == "frozen-transaction-id")
        #expect(model.fundingSyncFailed)
    }

    @Test
    func newPendingFundingOverridesPreviouslyAcceptedResult() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service })
        #expect(await model.submitFunding("review")?.value != nil)
        var pending = BsvMultisigFixtures.result(status: .pending)
        pending.transactionId = "new-pending-transaction"
        service.pendingResult = pending
        #expect(await model.loadFunding("multisig"))
        #expect(model.fundingResult?.status == .pending)
        #expect(model.fundingResult?.transactionId == "new-pending-transaction")
    }

    @Test
    func newPreparationClearsPreviousAcceptedResult() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service })
        #expect(await model.submitFunding("review")?.value != nil)
        #expect(await model.prepareFunding(walletId: "multisig", payerWalletId: "payer", amount: "0.00042")?.value != nil)
        #expect(model.fundingResult == nil)
        #expect(!model.fundingSyncFailed)
        #expect(model.fundingPreparation?.id == "review")
    }

    @Test
    func fundingFormForAnotherWalletClearsPreviousResult() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service })
        #expect(await model.submitFunding("review")?.value != nil)
        #expect(await model.loadFunding("other-wallet"))
        #expect(model.fundingResult == nil)
        #expect(!model.fundingSyncFailed)
    }
}
