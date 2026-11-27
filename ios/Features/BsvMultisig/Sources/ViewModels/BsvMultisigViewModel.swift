import Components
import Foundation
import func BsvHomeCore.bsvMultisigInvitationQrPayload
import func BsvHomeCore.bsvMultisigPaymailAlias
import func BsvHomeCore.bsvMultisigPaymentRequestQrPayload
import func BsvHomeCore.bsvMultisigScanRecipient
import enum BsvHomeCore.BsvHomeBsvMultisigActionIntent
import struct BsvHomeCore.BsvHomeBsvMultisigActionOutcome
import class BsvHomeCore.BsvHomeBsvMultisigActionScope
import struct BsvHomeCore.BsvHomeBsvMultisigActivity
import struct BsvHomeCore.BsvHomeBsvMultisigActivityItem
import enum BsvHomeCore.BsvHomeBsvMultisigError
import struct BsvHomeCore.BsvHomeBsvMultisigFundingPayer
import struct BsvHomeCore.BsvHomeBsvMultisigFundingPreparation
import struct BsvHomeCore.BsvHomeBsvMultisigFundingResult
import struct BsvHomeCore.BsvHomeBsvMultisigInvitation
import struct BsvHomeCore.BsvHomeBsvMultisigInvitePreview
import struct BsvHomeCore.BsvHomeBsvMultisigPaymailSettings
import struct BsvHomeCore.BsvHomeBsvMultisigProposal
import struct BsvHomeCore.BsvHomeBsvMultisigProposalReview
import protocol BsvHomeCore.BsvHomeBsvMultisigServiceProtocol
import struct BsvHomeCore.BsvHomeBsvMultisigSignerBinding
import struct BsvHomeCore.BsvHomeBsvMultisigWallet
import struct BsvHomeCore.BsvHomePaymentRecipient
import func BsvHomeCore.parseBsvMultisigInvitation
import Localization
import Observation
import UIKit

@Observable
@MainActor
public final class BsvMultisigViewModel {
    private enum ActivityPage: Hashable { case recent, older }
    private let actionLifetime: BsvMultisigActionLifetime
    private(set) var actionRevision: UInt = 0
    private(set) var actionOutcome: BsvHomeBsvMultisigActionOutcome?
    private(set) var actionSyncFailed = false
    public private(set) var wallets: [BsvHomeBsvMultisigWallet] = []
    public private(set) var proposals: [BsvHomeBsvMultisigProposal] = []
    public private(set) var pendingCount: UInt32 = 0
    public private(set) var isLoading = false
    public private(set) var isWorking = false
    public var errorMessage: String?
    public var isSharing = false
    public private(set) var shareFileURL: URL?
    public var isShowingCopy = false
    private(set) var uncertainCreationDraftId: String?
    private(set) var creationCandidates: [BsvHomeBsvMultisigWallet]?
    private(set) var fundingPayers: [BsvHomeBsvMultisigFundingPayer] = []
    private(set) var fundingPreparation: BsvHomeBsvMultisigFundingPreparation?
    private(set) var fundingResult: BsvHomeBsvMultisigFundingResult?
    private(set) var fundingSyncFailed = false
    private(set) var activities: [String: BsvHomeBsvMultisigActivity] = [:]
    private(set) var activityErrors: [String: String] = [:]
    private(set) var activityLoading: Set<String> = []
    private(set) var paymailSettings: [String: BsvHomeBsvMultisigPaymailSettings] = [:]
    private(set) var paymailErrors: [String: String] = [:]
    private(set) var paymailLoading: Set<String> = []
    private var activityRetries: [String: Set<ActivityPage>] = [:]
    private var activityTasks: [String: Task<Void, Never>] = [:]
    private var activityPages: [String: ActivityPage] = [:]
    private var paymailTasks: [String: Task<Void, Never>] = [:]

    private var service: (any BsvHomeBsvMultisigServiceProtocol)?
    private let makeService: @Sendable () throws -> any BsvHomeBsvMultisigServiceProtocol
    private let fiatBalance: @MainActor (String) -> String?
    private let walletName: @MainActor (String) -> String?
    private let syncPayer: @MainActor (String) async throws -> Void
    private let uptime: @MainActor () -> TimeInterval
    let amounts: BsvMultisigAmountFormatter
    private var fundingPreparedAt: TimeInterval?
    private var revision: UInt = 0
    private(set) var isCancelled = false

    public init(
        makeService: @escaping @Sendable () throws -> any BsvHomeBsvMultisigServiceProtocol,
        fiatBalance: @escaping @MainActor (String) -> String? = { _ in nil },
        walletName: @escaping @MainActor (String) -> String? = { _ in nil },
        syncPayer: @escaping @MainActor (String) async throws -> Void = { _ in },
        uptime: @escaping @MainActor () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
        locale: Locale = .current,
        cancelAuthorization: @escaping @MainActor (String) -> Void = { _ in },
    ) {
        self.makeService = makeService
        self.fiatBalance = fiatBalance
        self.walletName = walletName
        self.syncPayer = syncPayer
        self.uptime = uptime
        amounts = BsvMultisigAmountFormatter(locale: locale)
        actionLifetime = BsvMultisigActionLifetime(cancelAuthorization: cancelAuthorization)
    }

    func scanRecipient(_ code: String, onResult: @MainActor (BsvHomePaymentRecipient) -> Void) {
        guard !isCancelled else { return }
        cancelActions()
        errorMessage = nil
        do {
            try onResult(bsvMultisigScanRecipient(value: code))
        } catch {
            errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
        }
    }

    public func wallet(_ id: String) -> BsvHomeBsvMultisigWallet? {
        wallets.first { $0.id == id }
    }

    func proposal(_ id: String) -> BsvHomeBsvMultisigProposal? {
        proposals.first { $0.id == id }
    }

    func proposals(walletId: String) -> [BsvHomeBsvMultisigProposal] {
        proposals.filter { $0.walletId == walletId }
    }

    public func supportsAsset(_ assetId: String) -> Bool {
        service?.supportsAsset(assetId: assetId) ?? false
    }

    func amount(_ satoshis: String?, symbol: Bool = false) -> String {
        amounts.amount(satoshis, symbol: symbol)
    }

    func fiat(_ satoshis: String?) -> String? {
        satoshis.flatMap(fiatBalance)
    }

    func signingWalletName(_ wallet: BsvHomeBsvMultisigWallet) -> String? {
        wallet.localSignerWalletId.flatMap(walletName)
    }

    func signerName(_ binding: BsvHomeBsvMultisigSignerBinding) -> String {
        walletName(binding.walletId) ?? binding.address
    }

    var qrSizeLimit: Int { Int(service?.qrSizeLimit() ?? 0) }
    var fundingRemainingSeconds: Int {
        guard let preparation = fundingPreparation, let fundingPreparedAt else { return 0 }
        return max(0, Int(preparation.remainingSeconds) - Int(max(0, uptime() - fundingPreparedAt)))
    }

    func date(_ seconds: UInt64) -> String {
        let formatter = DateFormatter()
        formatter.locale = .current
        formatter.dateFormat = "MM-dd HH:mm"
        return formatter.string(from: Date(timeIntervalSince1970: TimeInterval(seconds)))
    }

    func activityItem(walletId: String, transactionId: String) -> BsvHomeBsvMultisigActivityItem? {
        activities[walletId]?.items.first { $0.transactionId == transactionId }
    }

    private func activeService() throws -> any BsvHomeBsvMultisigServiceProtocol {
        guard !isCancelled else { throw BsvHomeBsvMultisigError.Cancelled }
        if let service {
            return service
        }
        let service = try makeService()
        self.service = service
        return service
    }

    private func reload(_ service: any BsvHomeBsvMultisigServiceProtocol, expectedRevision: UInt) async throws {
        let snapshot = try await Task.detached(priority: .userInitiated) {
            try (service.getWallets(), service.getProposals(walletId: nil), service.pendingCount())
        }.value
        guard expectedRevision == revision, !Task.isCancelled else { return }
        wallets = snapshot.0
        proposals = snapshot.1
        pendingCount = snapshot.2
    }

    public func load() async {
        await refresh()
    }

    public func refresh() async {
        guard !isCancelled, !Task.isCancelled, !isLoading, !isWorking else { return }
        let currentRevision = revision
        isLoading = true
        defer { isLoading = false }
        do {
            let service = try activeService()
            try await service.load()
            guard currentRevision == revision, !Task.isCancelled else { return }
            try await reload(service, expectedRevision: currentRevision)
            guard currentRevision == revision else { return }
            errorMessage = nil
            if let actionOutcome, actionOutcome.submission == .submitted, !actionOutcome.syncFailed {
                actionSyncFailed = false
            }
        } catch {
            guard currentRevision == revision else { return }
            errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
            if let service {
                do { try await reload(service, expectedRevision: currentRevision) }
                catch {
                    guard currentRevision == revision else { return }
                    errorMessage = [errorMessage, BsvHomeBsvMultisigError.displayedMessage(for: error)].compactMap(\.self).joined(separator: "\n")
                }
            }
        }
    }

    public func cancel() {
        cancelActions()
        isCancelled = true
        revision &+= 1
        service?.cancel()
        for task in activityTasks.values {
            task.cancel()
        }
        for task in paymailTasks.values {
            task.cancel()
        }
        activityTasks = [:]
        activityPages = [:]
        paymailTasks = [:]
        isLoading = false
        isWorking = false
        fundingPreparation = nil
        fundingPreparedAt = nil
        activities = [:]
        activityErrors = [:]
        activityLoading = []
        activityRetries = [:]
        paymailSettings = [:]
        paymailErrors = [:]
        paymailLoading = []
    }

    public func cancelActions() {
        actionRevision &+= 1
        guard let action = actionLifetime.cancel() else { return }
        isWorking = false
        if let scope = action.scope {
            do { actionOutcome = try scope.outcome() }
            catch { errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error) }
        }
        fundingPreparation = nil
        fundingPreparedAt = nil
    }

    private func finishAction(_ id: UUID) {
        guard actionLifetime.finish(id) else { return }
        isWorking = false
    }

    private func startAction<T: Sendable>(
        intent: BsvHomeBsvMultisigActionIntent,
        navigation: NavigationPathState?,
        operation: @escaping @MainActor (any BsvHomeBsvMultisigServiceProtocol, BsvHomeBsvMultisigActionScope) async throws -> T,
        accept: @escaping @MainActor (T) async -> Void = { _ in },
        onComplete: @escaping @MainActor (T) -> Void = { _ in },
    ) -> Task<T?, Never>? {
        guard !isCancelled, !Task.isCancelled, !isWorking, !isLoading else { return nil }
        do {
            let service = try activeService()
            let scope = try service.beginAction(intent: intent)
            return startOwnedAction(service: service, scope: scope, navigation: navigation, operation: { try await operation(service, scope) }, accept: accept, onComplete: onComplete)
        } catch {
            errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
            return nil
        }
    }

    private func startOwnedAction<T: Sendable>(
        service: any BsvHomeBsvMultisigServiceProtocol,
        scope: BsvHomeBsvMultisigActionScope?,
        navigation: NavigationPathState?,
        operation: @escaping @MainActor () async throws -> T,
        accept: @escaping @MainActor (T) async -> Void,
        onComplete: @escaping @MainActor (T) -> Void,
    ) -> Task<T?, Never> {
        let id = actionLifetime.begin(scope: scope, navigation: navigation) { [weak self] in self?.cancelActions() }
        isWorking = true
        errorMessage = nil
        actionOutcome = nil
        actionSyncFailed = false
        let task = Task { [weak self] in
            guard let self else { scope?.cancel(); return nil as T? }
            return await withTaskCancellationHandler {
                guard actionLifetime.contains(id) else { return nil as T? }
                defer {
                    if actionLifetime.contains(id), Task.isCancelled {
                        cancelActions()
                    } else {
                        finishAction(id)
                    }
                }
                guard !Task.isCancelled else { return nil }
                do {
                    try scope?.check()
                    let value = try await operation()
                    guard actionLifetime.contains(id), !Task.isCancelled else { return nil }
                    if let scope {
                        do { actionOutcome = try scope.outcome(); actionSyncFailed = actionOutcome?.syncFailed == true }
                        catch { actionSyncFailed = true; errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error) }
                    }
                    await accept(value)
                    guard actionLifetime.contains(id), !Task.isCancelled else { return nil }
                    do { try await reload(service, expectedRevision: revision) }
                    catch { actionSyncFailed = true; errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error) }
                    guard actionLifetime.contains(id), !Task.isCancelled else { return nil }
                    finishAction(id)
                    onComplete(value)
                    return value
                } catch {
                    guard actionLifetime.contains(id), !Task.isCancelled else { return nil }
                    errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
                    if let scope {
                        do {
                            actionOutcome = try scope.outcome()
                            actionSyncFailed = actionOutcome?.syncFailed == true
                            if actionOutcome?.submission == .unknown {
                                errorMessage = Localized.multisigUncertain
                            }
                        } catch { errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error) }
                    }
                    if requiresRefresh(error) {
                        do {
                            try await service.load()
                            guard actionLifetime.contains(id), !Task.isCancelled else { return nil }
                            try await reload(service, expectedRevision: revision)
                        } catch {
                            guard actionLifetime.contains(id), !Task.isCancelled else { return nil }
                            errorMessage = [errorMessage, BsvHomeBsvMultisigError.displayedMessage(for: error)].compactMap(\.self).joined(separator: "\n")
                        }
                    }
                    return nil
                }
            } onCancel: {
                scope?.cancel()
                Task { @MainActor [weak self] in
                    guard self?.actionLifetime.contains(id) == true else { return }
                    self?.cancelActions()
                }
            }
        }
        actionLifetime.attach({ task.cancel() }, to: id)
        return task
    }

    private func perform<T>(_ action: (any BsvHomeBsvMultisigServiceProtocol) async throws -> T) async -> T? {
        guard !isCancelled, !Task.isCancelled, !isWorking, !isLoading else { return nil }
        let currentRevision = revision
        isWorking = true
        errorMessage = nil
        defer { isWorking = false }
        do {
            let service = try activeService()
            let value = try await action(service)
            guard currentRevision == revision, !Task.isCancelled else { return nil }
            try await reload(service, expectedRevision: currentRevision)
            guard currentRevision == revision, !Task.isCancelled else { return nil }
            return value
        } catch {
            guard currentRevision == revision, !Task.isCancelled else { return nil }
            if case let .CreationUncertain(draftId)? = error as? BsvHomeBsvMultisigError {
                uncertainCreationDraftId = draftId
            }
            errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
            if requiresRefresh(error), let service {
                do {
                    try await service.load()
                    try await reload(service, expectedRevision: currentRevision)
                } catch {
                    guard currentRevision == revision else { return nil }
                    errorMessage = [errorMessage, BsvHomeBsvMultisigError.displayedMessage(for: error)].compactMap(\.self).joined(separator: "\n")
                }
            }
            return nil
        }
    }

    private func requiresRefresh(_ error: any Error) -> Bool {
        switch error as? BsvHomeBsvMultisigError {
        case .ConcurrentUpdate, .ConfigurationConflict, .MissingRecord: true
        case let .Api(code), let .InvalidPackage(code): ["STALE_VERSION", "SIGNATURE_CONFLICT", "FINAL_TRANSACTION_MISMATCH", "BROADCAST_CONFLICT", "INVITATION_WALLET_MISMATCH", "INVALID_INVITE_REVIEW", "WALLET_NOT_FOUND", "PROPOSAL_NOT_FOUND", "PROPOSAL_CLOSED", "PROPOSAL_NOT_READY", "INVITATION_CLOSED", "CREATOR_REQUIRED"].contains(code)
        default: false
        }
    }
}

extension BsvMultisigViewModel {
    func beginCreation(name: String, threshold: UInt32, memberNames: [String], signerWalletId: String?) async -> BsvMultisigCreationReview? {
        guard let draft = await perform({ service in
            let identityId: String = if let signerWalletId {
                signerWalletId
            } else {
                try await service.currentSigningIdentity().walletId
            }
            return try await service.beginCreation(name: name, threshold: threshold, memberNames: memberNames, signerWalletId: identityId)
        }) else { return nil }
        uncertainCreationDraftId = nil
        creationCandidates = nil
        return BsvMultisigCreationReview(draft: draft)
    }

    func submitCreation(_ review: BsvMultisigCreationReview) async -> String? {
        await perform { try await $0.submitCreation(draftId: review.draft.id).id }
    }

    func reconcileCreation(_ review: BsvMultisigCreationReview) async {
        guard !isWorking, !isLoading else { return }
        creationCandidates = nil
        creationCandidates = await perform { try await $0.reconcileCreation(draftId: review.draft.id) }
    }

    func acceptCreation(_ review: BsvMultisigCreationReview, walletId: String) async -> String? {
        await perform { try await $0.acceptCreation(draftId: review.draft.id, walletId: walletId).id }
    }

    func invitation(_ walletId: String) async -> BsvHomeBsvMultisigInvitation? {
        await perform { try await $0.invitation(walletId: walletId) }
    }

    func previewInvitation(_ value: String) async -> BsvHomeBsvMultisigInvitePreview? {
        guard !isWorking, !isLoading else { return nil }
        do {
            let token = try parseBsvMultisigInvitation(value: value)
            return await perform { try await $0.previewInvitation(token: token) }
        } catch {
            errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
            return nil
        }
    }

    func invitationQrPayload(_ token: String) -> String? {
        do {
            return try bsvMultisigInvitationQrPayload(token: token)
        } catch {
            errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
            return nil
        }
    }

    func join(_ preview: BsvHomeBsvMultisigInvitePreview) async -> String? {
        await perform { try await $0.joinInvitation(preview: preview).id }
    }

    func approveConfiguration(_ walletId: String, expectedVersion: UInt64) async -> Bool {
        await perform { try await $0.approveConfiguration(walletId: walletId, expectedVersion: expectedVersion) } != nil
    }

    func reviewSnapshot(_ proposalId: String) async -> BsvHomeBsvMultisigProposalReview? {
        let currentRevision = revision
        do {
            let service = try activeService()
            let snapshot = try await Task.detached(priority: .userInitiated) { try service.proposalReviewSnapshot(proposalId: proposalId) }.value
            guard currentRevision == revision, !Task.isCancelled else { return nil }
            return snapshot
        } catch {
            guard currentRevision == revision, !Task.isCancelled else { return nil }
            errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
            return nil
        }
    }

    @discardableResult
    func createProposal(walletId: String, destination: String, amount: String, memo: String, navigation: NavigationPathState? = nil, onComplete: @escaping @MainActor (String) -> Void = { _ in }) -> Task<String?, Never>? {
        do {
            let amount = try amounts.encoded(amount)
            return startAction(intent: .prepareProposal(walletId: walletId, destination: destination, amountBsv: amount, memo: memo), navigation: navigation, operation: {
                try await $0.prepareProposal(walletId: walletId, destination: destination, amountBsv: amount, memo: memo, action: $1).id
            }, onComplete: onComplete)
        } catch { errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error); return nil }
    }

    @discardableResult
    func sign(_ review: BsvHomeBsvMultisigProposalReview, navigation: NavigationPathState? = nil, onComplete: @escaping @MainActor (BsvHomeBsvMultisigProposal) -> Void = { _ in }) -> Task<BsvHomeBsvMultisigProposal?, Never>? {
        startAction(intent: .signProposal(proposalId: review.proposal.id, expectedVersion: review.proposal.version, reviewId: review.reviewId), navigation: navigation, operation: {
            try await $0.signProposal(proposalId: review.proposal.id, expectedVersion: review.proposal.version, action: $1)
        }, onComplete: onComplete)
    }

    @discardableResult
    func broadcast(_ review: BsvHomeBsvMultisigProposalReview, navigation: NavigationPathState? = nil, onComplete: @escaping @MainActor (BsvHomeBsvMultisigProposal) -> Void = { _ in }) -> Task<BsvHomeBsvMultisigProposal?, Never>? {
        startAction(intent: .broadcastProposal(proposalId: review.proposal.id, expectedVersion: review.proposal.version, reviewId: review.reviewId), navigation: navigation, operation: {
            try await $0.broadcastProposal(proposalId: review.proposal.id, expectedVersion: review.proposal.version, action: $1)
        }, onComplete: onComplete)
    }

    @discardableResult
    func withdraw(_ review: BsvHomeBsvMultisigProposalReview, navigation: NavigationPathState? = nil) -> Task<BsvHomeBsvMultisigProposal?, Never>? {
        startAction(intent: .withdrawProposal(proposalId: review.proposal.id, expectedVersion: review.proposal.version, reviewId: review.reviewId), navigation: navigation, operation: {
            try await $0.withdrawProposal(proposalId: review.proposal.id, expectedVersion: review.proposal.version, action: $1)
        })
    }

    func refreshProposal(_ proposalId: String) async {
        _ = await perform { try await $0.refreshProposal(proposalId: proposalId) }
    }

    func refreshWallet(_ walletId: String) async {
        _ = await perform { try await $0.refreshWallet(walletId: walletId) }
    }

    func refreshWalletActivity(_ walletId: String) async {
        await refreshWallet(walletId)
        guard !Task.isCancelled, wallet(walletId)?.status == .active else { return }
        await refreshActivity(walletId)
    }

    public func refreshActivity(_ walletId: String) async {
        await loadActivity(walletId, more: false)
    }

    func loadMoreActivity(_ walletId: String) async {
        await loadActivity(walletId, more: true)
    }

    func retryActivity(_ walletId: String) async {
        await loadActivity(walletId, more: activityRetries[walletId]?.contains(.older) == true && activityRetries[walletId]?.contains(.recent) != true)
    }

    private func loadActivity(_ walletId: String, more: Bool) async {
        guard !isCancelled, !Task.isCancelled else { return }
        if let task = activityTasks[walletId] {
            let page = activityPages[walletId]
            await task.value
            if !isCancelled, !Task.isCancelled, page != (more ? .older : .recent) {
                await loadActivity(walletId, more: more)
            }
            return
        }
        let currentRevision = revision
        activityLoading.insert(walletId)
        let task = Task { await fetchActivity(walletId, more: more, expectedRevision: currentRevision) }
        activityPages[walletId] = more ? .older : .recent
        activityTasks[walletId] = task
        await task.value
    }

    private func fetchActivity(_ walletId: String, more: Bool, expectedRevision currentRevision: UInt) async {
        guard currentRevision == revision, !Task.isCancelled else { return }
        defer {
            if revision == currentRevision {
                activityTasks[walletId] = nil
                activityPages[walletId] = nil
                activityLoading.remove(walletId)
            }
        }
        do {
            let service = try activeService()
            let snapshot = try await (more ? service.loadMoreActivity(walletId: walletId) : service.refreshActivity(walletId: walletId))
            guard currentRevision == revision, !Task.isCancelled else { return }
            activities[walletId] = snapshot
            activityRetries[walletId]?.remove(more ? .older : .recent)
            if !snapshot.isStale {
                activityRetries[walletId] = nil
            }
            activityErrors[walletId] = snapshot.isStale ? Localized.multisigErrorProvider : nil
        } catch {
            guard currentRevision == revision, !Task.isCancelled else { return }
            activityRetries[walletId, default: []].insert(more ? .older : .recent)
            activityErrors[walletId] = BsvHomeBsvMultisigError.displayedMessage(for: error)
            if let service {
                do {
                    let cached = try await Task.detached(priority: .userInitiated) { try service.getActivity(walletId: walletId) }.value
                    guard currentRevision == revision, !Task.isCancelled else { return }
                    activities[walletId] = cached
                } catch {
                    guard currentRevision == revision, !Task.isCancelled else { return }
                    activityErrors[walletId] = [activityErrors[walletId], BsvHomeBsvMultisigError.displayedMessage(for: error)].compactMap(\.self).joined(separator: "\n")
                }
            }
        }
    }

    func paymentPayload(_ walletId: String, amount: String) async throws -> String {
        try Task.checkCancellation()
        let currentRevision = revision
        let service = try activeService()
        let encoded = try amounts.encoded(amount)
        let payload = try await Task.detached(priority: .userInitiated) {
            try bsvMultisigPaymentRequestQrPayload(request: service.paymentRequest(walletId: walletId, amount: encoded))
        }.value
        try Task.checkCancellation()
        guard currentRevision == revision else { throw CancellationError() }
        return payload
    }

    func loadPaymail(_ walletId: String) async {
        guard !isCancelled, !Task.isCancelled else { return }
        if let task = paymailTasks[walletId] {
            await task.value
            return
        }
        let currentRevision = revision
        paymailLoading.insert(walletId)
        let task = Task { await fetchPaymail(walletId, expectedRevision: currentRevision) }
        paymailTasks[walletId] = task
        await task.value
    }

    private func fetchPaymail(_ walletId: String, expectedRevision currentRevision: UInt) async {
        guard currentRevision == revision, !Task.isCancelled else { return }
        defer {
            if revision == currentRevision {
                paymailTasks[walletId] = nil
                paymailLoading.remove(walletId)
            }
        }
        do {
            let settings = try await activeService().loadPaymailSettings(walletId: walletId)
            guard currentRevision == revision, !Task.isCancelled else { return }
            paymailSettings[walletId] = settings
            paymailErrors[walletId] = nil
        } catch {
            guard currentRevision == revision, !Task.isCancelled else { return }
            paymailErrors[walletId] = BsvHomeBsvMultisigError.displayedMessage(for: error)
        }
    }

    func canonicalPaymailAlias(_ alias: String) -> String? {
        do { return try bsvMultisigPaymailAlias(value: alias) }
        catch { errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error); return nil }
    }

    @discardableResult
    func updatePaymail(_ settings: BsvHomeBsvMultisigPaymailSettings, alias: String, active: Bool, navigation: NavigationPathState? = nil) -> Task<BsvHomeBsvMultisigPaymailSettings?, Never>? {
        guard settings.canManage, !paymailLoading.contains(settings.walletId), paymailSettings[settings.walletId] == settings else { return nil }
        let task = startAction(intent: .updatePaymailSettings(walletId: settings.walletId, alias: alias, active: active, expectedVersion: settings.version, configurationHash: settings.configurationHash), navigation: navigation, operation: {
            try await $0.updatePaymailSettings(settings: settings, alias: alias, active: active, action: $1)
        }, accept: { [weak self] updated in
            self?.paymailSettings[settings.walletId] = updated
        })
        if task != nil {
            paymailSettings[settings.walletId] = nil
            paymailErrors[settings.walletId] = nil
        }
        return task
    }

    func receiveRequest(_ walletId: String) async -> String? {
        let currentRevision = revision
        do {
            let service = try activeService()
            let request = try await Task.detached(priority: .userInitiated) { try service.receiveRequest(walletId: walletId) }.value
            guard currentRevision == revision else { return nil }
            return request
        } catch {
            guard currentRevision == revision else { return nil }
            errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
            return nil
        }
    }

    func exportBackup(_ walletId: String) async {
        let currentRevision = revision
        do {
            let service = try activeService()
            let configuration = try await Task.detached(priority: .userInitiated) { try service.exportConfiguration(walletId: walletId) }.value
            guard currentRevision == revision else { return }
            share(configuration, filename: "wowpay-bsv-multisig-\(walletId).json")
        } catch {
            guard currentRevision == revision else { return }
            errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
        }
    }

    func loadFunding(_ walletId: String) async -> Bool {
        fundingPreparation = nil
        fundingPreparedAt = nil
        if fundingResult?.memberWalletId != walletId {
            fundingResult = nil
            fundingSyncFailed = false
        }
        guard let loaded = await perform({ service in
            let pending = try await service.pendingFunding(walletId: walletId)
            let payers = try await service.fundingPayers()
            return (pending, payers)
        }) else { return false }
        if let pending = loaded.0 {
            fundingResult = pending
            fundingSyncFailed = false
        }
        fundingPayers = loaded.1
        return true
    }

    @discardableResult
    func prepareFunding(walletId: String, payerWalletId: String, amount: String, navigation: NavigationPathState? = nil, onComplete: @escaping @MainActor (BsvHomeBsvMultisigFundingPreparation) -> Void = { _ in }) -> Task<BsvHomeBsvMultisigFundingPreparation?, Never>? {
        guard !isCancelled, !Task.isCancelled, !isWorking, !isLoading else { return nil }
        do {
            let amount = try amounts.encoded(amount)
            let service = try activeService()
            return startOwnedAction(service: service, scope: nil, navigation: navigation, operation: {
                try await service.prepareFunding(walletId: walletId, payerWalletId: payerWalletId, amountBsv: amount)
            }, accept: { [weak self] preparation in
                self?.fundingPreparation = preparation
                self?.fundingPreparedAt = self?.uptime()
                self?.fundingResult = nil
                self?.fundingSyncFailed = false
            }, onComplete: onComplete)
        } catch { errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error); return nil }
    }

    @discardableResult
    func submitFunding(_ preparationId: String, navigation: NavigationPathState? = nil, onComplete: @escaping @MainActor (BsvHomeBsvMultisigFundingResult) -> Void = { _ in }) -> Task<BsvHomeBsvMultisigFundingResult?, Never>? {
        let task = startAction(intent: .submitFunding(preparationId: preparationId), navigation: navigation, operation: {
            try await $0.submitFunding(preparationId: preparationId, action: $1)
        }, accept: { [weak self] result in
            self?.fundingResult = result
            await self?.syncFundingPayer(result)
        }, onComplete: onComplete)
        if task != nil {
            fundingPreparation = nil; fundingPreparedAt = nil
        }
        return task
    }

    func loadFundingResult(_ walletId: String) async {
        if let result = await perform({ try await $0.fundingResult(walletId: walletId) }) {
            fundingResult = result
            if let result {
                await syncFundingPayer(result)
            }
        }
    }

    @discardableResult
    func retryFunding(_ walletId: String, navigation: NavigationPathState? = nil) -> Task<BsvHomeBsvMultisigFundingResult?, Never>? {
        startAction(intent: .retryFunding(walletId: walletId), navigation: navigation, operation: {
            try await $0.retryFunding(walletId: walletId, action: $1)
        }, accept: { [weak self] result in
            self?.fundingResult = result
            await self?.syncFundingPayer(result)
        })
    }

    func retryFundingSync() async {
        guard let walletId = fundingResult?.memberWalletId else { return }
        guard let result = await perform({ try await $0.retryFundingSync(walletId: walletId) }) else { return }
        fundingResult = result
        await syncFundingPayer(result)
    }

    private func syncFundingPayer(_ result: BsvHomeBsvMultisigFundingResult) async {
        guard result.status != .pending else { return }
        let expectedActionRevision = actionRevision
        fundingSyncFailed = result.status == .syncFailed
        do { try await syncPayer(result.payerWalletId) }
        catch {
            guard expectedActionRevision == actionRevision, !isCancelled, !Task.isCancelled else { return }
            fundingSyncFailed = true
        }
    }

    func copy(_ value: String) {
        UIPasteboard.general.string = value
        isShowingCopy = true
    }

    func share(_ value: String, filename: String = "wowpay-bsv-multisig.json") {
        do {
            clearShare()
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("WowPayMultisigShare", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            var protectedDirectory = directory
            var values = URLResourceValues()
            values.isExcludedFromBackup = true
            try protectedDirectory.setResourceValues(values)
            let url = directory.appendingPathComponent(filename)
            try Data(value.utf8).write(to: url, options: [.atomic, .completeFileProtection])
            shareFileURL = url
            isSharing = true
        } catch { errorMessage = Localized.multisigServiceError("EXPORT_FAILED") }
    }

    func clearShare() {
        do {
            if let shareFileURL {
                try FileManager.default.removeItem(at: shareFileURL)
            }
            shareFileURL = nil
        } catch { errorMessage = Localized.multisigServiceError("EXPORT_FAILED") }
    }
}
