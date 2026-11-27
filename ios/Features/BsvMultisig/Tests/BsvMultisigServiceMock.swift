import BsvHomeCorePrimitivesTestKit
import Foundation
import BsvHomeCore

final class BsvMultisigServiceMock: BsvHomeBsvMultisigServiceProtocol, @unchecked Sendable {
    var actionScopeFactory: (@Sendable () throws -> BsvHomeBsvMultisigActionScope)?
    var onPrepareFunding: (@Sendable () async throws -> BsvHomeBsvMultisigFundingPreparation)?
    var onAction: (@Sendable (BsvHomeBsvMultisigActionScope) async throws -> Void)?
    var onRefreshProposal: (@Sendable () async throws -> BsvHomeBsvMultisigProposal)?
    var snapshotError: (any Error)?
    private var recordedIntents: [BsvHomeBsvMultisigActionIntent] = []
    private var recordedScopes: [BsvHomeBsvMultisigActionScope] = []
    var actionIntents: [BsvHomeBsvMultisigActionIntent] { lock.withLock { recordedIntents } }
    var actionScopes: [BsvHomeBsvMultisigActionScope] { lock.withLock { recordedScopes } }
    var operationError: (any Error)?
    var creationSubmitError: (any Error)?
    var creationCandidates: [BsvHomeBsvMultisigWallet] = []
    var walletRecords: [BsvHomeBsvMultisigWallet]?
    var proposalRecords: [BsvHomeBsvMultisigProposal]?
    var activityRecords: [String: BsvHomeBsvMultisigActivity] = [:]
    var onRefreshActivity: (@Sendable (String) async throws -> BsvHomeBsvMultisigActivity)?
    var onLoadMoreActivity: (@Sendable (String) async throws -> BsvHomeBsvMultisigActivity)?
    var onLoadPaymail: (@Sendable (String) async throws -> BsvHomeBsvMultisigPaymailSettings)?
    var onUpdatePaymail: (@Sendable (BsvHomeBsvMultisigPaymailSettings, String, Bool) async throws -> BsvHomeBsvMultisigPaymailSettings)?
    var creationStatus: BsvHomeBsvMultisigCreationStatus = .ready
    var onReconcileCreation: (@Sendable () async throws -> [BsvHomeBsvMultisigWallet])?
    let result: BsvHomeBsvMultisigFundingResult
    var pendingResult: BsvHomeBsvMultisigFundingResult?
    let loadAction: @Sendable () async throws -> Void
    let recordsAreEmpty: Bool
    private let lock = NSLock()
    private var recorded: [String] = []
    var calls: [String] { lock.withLock { recorded } }

    init(result: BsvHomeBsvMultisigFundingResult = BsvMultisigFixtures.result(), pendingResult: BsvHomeBsvMultisigFundingResult? = nil, recordsAreEmpty: Bool = false, loadAction: @escaping @Sendable () async throws -> Void = {}) {
        self.result = result
        self.pendingResult = pendingResult
        self.loadAction = loadAction
        self.recordsAreEmpty = recordsAreEmpty
    }

    static func scoped(result: BsvHomeBsvMultisigFundingResult = BsvMultisigFixtures.result(), pendingResult: BsvHomeBsvMultisigFundingResult? = nil, recordsAreEmpty: Bool = false, loadAction: @escaping @Sendable () async throws -> Void = {}) async throws -> BsvMultisigServiceMock {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let service = BsvMultisigServiceMock(result: result, pendingResult: pendingResult, recordsAreEmpty: recordsAreEmpty, loadAction: loadAction)
        service.actionScopeFactory = { try fixture.beginRetryFunding() }
        return service
    }

    func beginAction(intent: BsvHomeBsvMultisigActionIntent) throws -> BsvHomeBsvMultisigActionScope {
        guard let actionScopeFactory else { throw BsvHomeBsvMultisigError.MissingRecord }
        let scope = try actionScopeFactory()
        lock.withLock {
            recordedIntents.append(intent)
            recordedScopes.append(scope)
        }
        return scope
    }

    private func record(_ action: String) {
        lock.withLock { recorded.append(action) }
    }

    private func checkOperation() throws {
        if let operationError {
            throw operationError
        }
    }

    func load() async throws {
        record("load"); try await loadAction()
    }

    func cancel() {
        record("cancel")
    }

    func getWallets() throws -> [BsvHomeBsvMultisigWallet] {
        if let snapshotError {
            throw snapshotError
        }
        return walletRecords ?? (recordsAreEmpty ? [] : [BsvMultisigFixtures.wallet()])
    }

    func getWallet(walletId: String) throws -> BsvHomeBsvMultisigWallet {
        guard let wallet = try getWallets().first(where: { $0.id == walletId }) else { throw BsvHomeBsvMultisigError.MissingRecord }
        return wallet
    }

    func getProposals(walletId _: String?) throws -> [BsvHomeBsvMultisigProposal] {
        proposalRecords ?? (recordsAreEmpty ? [] : [BsvMultisigFixtures.proposal()])
    }

    func getProposal(proposalId _: String) throws -> BsvHomeBsvMultisigProposal {
        BsvMultisigFixtures.proposal()
    }

    func pendingCount() throws -> UInt32 {
        1
    }

    func supportsAsset(assetId: String) -> Bool {
        assetId == "bitcoinsv"
    }

    func relatedMultisigCount(signerWalletId _: String) throws -> UInt32 {
        1
    }

    func qrSizeLimit() -> UInt64 {
        2000
    }

    func currentSigningIdentity() async throws -> BsvHomeBsvMultisigSignerBinding {
        BsvHomeBsvMultisigSignerBinding(walletId: "member", publicKey: "member-public-key", address: "member-address")
    }

    func beginCreation(name: String, threshold: UInt32, memberNames: [String], signerWalletId: String) async throws -> BsvHomeBsvMultisigCreationDraft {
        record("begin:\(signerWalletId):\(threshold):\(memberNames.count)")
        try checkOperation()
        return BsvHomeBsvMultisigCreationDraft(id: "creation-draft", name: name, threshold: threshold, memberNames: memberNames,
                                           signingIdentity: BsvHomeBsvMultisigSignerBinding(walletId: signerWalletId, publicKey: "member-public-key", address: "member-address"), status: creationStatus)
    }

    func submitCreation(draftId: String) async throws -> BsvHomeBsvMultisigWallet {
        record("submit:\(draftId)")
        try checkOperation()
        if let creationSubmitError {
            throw creationSubmitError
        }
        return BsvMultisigFixtures.wallet()
    }

    func reconcileCreation(draftId: String) async throws -> [BsvHomeBsvMultisigWallet] {
        record("reconcile:\(draftId)")
        try checkOperation()
        return try await onReconcileCreation?() ?? creationCandidates
    }

    func acceptCreation(draftId: String, walletId: String) async throws -> BsvHomeBsvMultisigWallet {
        record("accept:\(draftId):\(walletId)")
        try checkOperation()
        return BsvMultisigFixtures.wallet(id: walletId)
    }

    func invitation(walletId: String) async throws -> BsvHomeBsvMultisigInvitation {
        BsvHomeBsvMultisigInvitation(walletId: walletId, token: String(repeating: "A", count: 43), expiresAt: 10000)
    }

    func previewInvitation(token: String) async throws -> BsvHomeBsvMultisigInvitePreview {
        record("preview:\(token)")
        return try await BsvHomeBsvMultisigInvitePreview(wallet: BsvMultisigFixtures.wallet(), token: token, expiresAt: 10000, signingIdentity: currentSigningIdentity())
    }

    func joinInvitation(preview: BsvHomeBsvMultisigInvitePreview) async throws -> BsvHomeBsvMultisigWallet {
        record("join:\(preview.wallet.version)"); try checkOperation(); return preview.wallet
    }

    func approveConfiguration(walletId _: String, expectedVersion: UInt64) async throws -> BsvHomeBsvMultisigWallet {
        record("confirm:\(expectedVersion)"); try checkOperation(); return BsvMultisigFixtures.wallet()
    }

    func prepareProposal(walletId _: String, destination _: String, amountBsv: String, memo _: String, action: BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigProposal {
        record("prepare:\(amountBsv)"); try checkOperation(); try await onAction?(action); return BsvMultisigFixtures.proposal()
    }

    func proposalReviewSnapshot(proposalId _: String) throws -> BsvHomeBsvMultisigProposalReview {
        BsvMultisigFixtures.review()
    }

    func proposalReview(proposalId _: String) throws -> [BsvHomeBsvMultisigField] {
        [BsvHomeBsvMultisigField(title: "change", value: "57800")]
    }

    func signProposal(proposalId _: String, expectedVersion: UInt64, action: BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigProposal {
        record("sign:\(expectedVersion)"); try checkOperation(); try await onAction?(action); return BsvMultisigFixtures.proposal()
    }

    func broadcastProposal(proposalId _: String, expectedVersion: UInt64, action: BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigProposal {
        record("broadcast:\(expectedVersion)"); try checkOperation(); try await onAction?(action); return BsvMultisigFixtures.proposal()
    }

    func withdrawProposal(proposalId _: String, expectedVersion: UInt64, action: BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigProposal {
        record("withdraw:\(expectedVersion)"); try checkOperation(); try await onAction?(action); return BsvMultisigFixtures.proposal()
    }

    func refreshWallet(walletId _: String) async throws -> BsvHomeBsvMultisigWallet {
        record("refresh-wallet"); return BsvMultisigFixtures.wallet()
    }

    func refreshProposal(proposalId _: String) async throws -> BsvHomeBsvMultisigProposal {
        record("refresh-proposal")
        return try await onRefreshProposal?() ?? BsvMultisigFixtures.proposal()
    }

    func exportConfiguration(walletId _: String) throws -> String {
        "public-configuration"
    }

    func receiveRequest(walletId _: String) throws -> String {
        "public-configuration"
    }

    func getActivity(walletId: String) throws -> BsvHomeBsvMultisigActivity {
        record("get-activity:\(walletId)")
        return activityRecords[walletId] ?? BsvHomeBsvMultisigActivity(walletId: walletId, items: [], nextCursor: "", isStale: false)
    }

    func refreshActivity(walletId: String) async throws -> BsvHomeBsvMultisigActivity {
        record("refresh-activity:\(walletId)")
        try checkOperation()
        return try await onRefreshActivity?(walletId) ?? getActivity(walletId: walletId)
    }

    func loadMoreActivity(walletId: String) async throws -> BsvHomeBsvMultisigActivity {
        record("more-activity:\(walletId)")
        try checkOperation()
        return try await onLoadMoreActivity?(walletId) ?? getActivity(walletId: walletId)
    }

    func paymentRequest(walletId: String, amount: String) throws -> BsvHomeBsvMultisigPaymentRequest {
        record("payment-request:\(walletId):\(amount)")
        try checkOperation()
        return try BsvMultisigFixtures.paymentRequest(amount: amount)
    }

    func loadPaymailSettings(walletId: String) async throws -> BsvHomeBsvMultisigPaymailSettings {
        record("load-paymail:\(walletId)")
        try checkOperation()
        return try await onLoadPaymail?(walletId) ?? BsvMultisigFixtures.paymail(walletId: walletId)
    }

    func updatePaymailSettings(settings: BsvHomeBsvMultisigPaymailSettings, alias: String, active: Bool, action: BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigPaymailSettings {
        record("update-paymail:\(settings.walletId):\(settings.version)")
        try checkOperation()
        try await onAction?(action)
        if let onUpdatePaymail {
            return try await onUpdatePaymail(settings, alias, active)
        }
        var updated = settings
        updated.alias = alias
        updated.handle = alias + "@" + settings.domain
        updated.active = active
        updated.version += 1
        return updated
    }

    func fundingPayers() async throws -> [BsvHomeBsvMultisigFundingPayer] {
        [BsvHomeBsvMultisigFundingPayer(walletId: "payer", name: "Payer", address: "payer-address")]
    }

    func pendingFunding(walletId _: String) async throws -> BsvHomeBsvMultisigFundingResult? {
        pendingResult
    }

    func fundingResult(walletId _: String) async throws -> BsvHomeBsvMultisigFundingResult? {
        result
    }

    func prepareFunding(walletId _: String, payerWalletId: String, amountBsv: String) async throws -> BsvHomeBsvMultisigFundingPreparation {
        record("prepare-funding:\(payerWalletId):\(amountBsv)"); try checkOperation(); return try await onPrepareFunding?() ?? BsvMultisigFixtures.preparation()
    }

    func submitFunding(preparationId: String, action: BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigFundingResult {
        record("submit-funding:\(preparationId)"); try checkOperation(); try await onAction?(action); return result
    }

    func retryFunding(walletId _: String, action: BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigFundingResult {
        record("retry-funding"); try checkOperation(); try await onAction?(action); return result
    }

    func retryFundingSync(walletId _: String) async throws -> BsvHomeBsvMultisigFundingResult {
        record("sync-funding"); try checkOperation(); return BsvMultisigFixtures.result()
    }
}
