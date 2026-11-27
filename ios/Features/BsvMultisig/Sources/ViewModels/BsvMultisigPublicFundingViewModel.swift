import Components
import Foundation
import enum BsvHomeCore.BsvHomeBsvMultisigActionIntent
import struct BsvHomeCore.BsvHomeBsvMultisigActionOutcome
import class BsvHomeCore.BsvHomeBsvMultisigActionScope
import enum BsvHomeCore.BsvHomeBsvMultisigError
import struct BsvHomeCore.BsvHomeBsvMultisigFundingPreparation
import struct BsvHomeCore.BsvHomeBsvMultisigFundingResult
import struct BsvHomeCore.BsvHomeBsvMultisigPaymentRequest
import protocol BsvHomeCore.BsvHomeBsvMultisigPublicFundingServiceProtocol
import struct BsvHomeCore.Wallet
import Localization
import Observation

@Observable
@MainActor
public final class BsvMultisigPublicFundingViewModel {
    public let id = UUID()
    public let payer: Wallet
    public let entry: BsvMultisigPublicFundingEntry
    let amounts: BsvMultisigAmountFormatter
    private let makeService: @Sendable () throws -> any BsvHomeBsvMultisigPublicFundingServiceProtocol
    private let syncPayer: @MainActor (String) async throws -> Void
    private let uptime: @MainActor () -> TimeInterval
    private let actionLifetime: BsvMultisigActionLifetime
    private var service: (any BsvHomeBsvMultisigPublicFundingServiceProtocol)?
    private var cancelled = false
    private var revision: UInt = 0
    private var preparedAt: TimeInterval?
    private(set) var readTask: Task<Void, Never>?
    private(set) var preparation: BsvHomeBsvMultisigFundingPreparation?
    private(set) var result: BsvHomeBsvMultisigFundingResult?
    private(set) var recoveryState: StateViewType<BsvMultisigFundingRecovery> = .noData
    private(set) var hasReviewed = false
    private(set) var isWorking = false
    private(set) var outcome: BsvHomeBsvMultisigActionOutcome?
    private(set) var syncFailed = false
    var errorMessage: String?

    public init(
        entry: BsvMultisigPublicFundingEntry,
        payer: Wallet,
        makeService: @escaping @Sendable () throws -> any BsvHomeBsvMultisigPublicFundingServiceProtocol,
        syncPayer: @escaping @MainActor (String) async throws -> Void = { _ in },
        uptime: @escaping @MainActor () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
        locale: Locale = .current,
        cancelAuthorization: @escaping @MainActor (String) -> Void = { _ in },
    ) {
        self.entry = entry
        self.payer = payer
        self.makeService = makeService
        self.syncPayer = syncPayer
        self.uptime = uptime
        amounts = BsvMultisigAmountFormatter(locale: locale)
        actionLifetime = BsvMultisigActionLifetime(cancelAuthorization: cancelAuthorization)
    }

    var request: BsvHomeBsvMultisigPaymentRequest? { entry.request }

    var remainingSeconds: Int {
        guard let preparation, let preparedAt else { return 0 }
        return max(0, Int(preparation.remainingSeconds) - Int(max(0, uptime() - preparedAt)))
    }

    var requiresRecovery: Bool {
        outcome?.submission == .unknown || outcome?.submission == .submitted
    }

    func canPrepare(amount: String) -> Bool {
        request != nil && recoveryState.value != nil && result == nil && !requiresRecovery && !amount.isEmpty && !isWorking && readTask == nil && !cancelled
    }

    func canSubmit(_ id: String) -> Bool {
        hasReviewed && preparation?.id == id && remainingSeconds > 0 && !isWorking && readTask == nil && !cancelled
    }

    func setReviewed(_ reviewed: Bool, preparationId: String) {
        guard preparation?.id == preparationId, !cancelled else { return }
        hasReviewed = reviewed
    }

    func startLoading() {
        guard recoveryState.isNoData else { return }
        reloadResults()
    }

    func reloadResults() {
        guard !cancelled, !isWorking, readTask == nil else { return }
        let current = revision
        errorMessage = nil
        recoveryState = .loading
        readTask = Task { [weak self] in
            guard let self else { return }
            defer {
                if current == revision {
                    readTask = nil
                    if Task.isCancelled, recoveryState.isLoading {
                        recoveryState = .noData
                    }
                }
            }
            guard !cancelled, !Task.isCancelled, current == revision else { return }
            do {
                let values = try await recoverySnapshot(activeService())
                guard !cancelled, !Task.isCancelled, current == revision else { return }
                recoveryState = .data(values)
            } catch {
                guard !cancelled, !Task.isCancelled, current == revision else { return }
                recoveryState = .error(error)
                errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
            }
        }
    }

    @discardableResult
    func prepare(amount: String, navigation: NavigationPathState?, onComplete: @escaping @MainActor (BsvHomeBsvMultisigFundingPreparation) -> Void) -> Task<Void, Never>? {
        guard canPrepare(amount: amount), case let .payment(source) = entry else { return nil }
        do {
            let service = try activeService()
            let encoded = try amounts.encoded(amount)
            return start(scope: nil, navigation: navigation) { [weak self] in
                let review = try await service.prepareFunding(source: source, amountBsv: encoded)
                guard let self, !Task.isCancelled else { return }
                preparation = review
                preparedAt = uptime()
                result = nil
                hasReviewed = false
            } onComplete: { [weak self] in
                if let review = self?.preparation {
                    onComplete(review)
                }
            }
        } catch { errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error); return nil }
    }

    @discardableResult
    func submit(_ id: String, navigation: NavigationPathState?, onComplete: @escaping @MainActor (BsvHomeBsvMultisigFundingResult) -> Void) -> Task<Void, Never>? {
        guard canSubmit(id) else { return nil }
        return financial(intent: .submitFunding(preparationId: id), navigation: navigation, operation: { try await $0.submitFunding(preparationId: id, action: $1) }, onComplete: onComplete)
    }

    func selectRecovery(_ operationId: String) -> Bool {
        guard !cancelled, !isWorking, let selected = recoveryState.value?.results.first(where: { $0.operationId == operationId }) else { return false }
        preparation = nil
        preparedAt = nil
        hasReviewed = false
        result = selected
        outcome = nil
        syncFailed = selected.status == .syncFailed
        return true
    }

    @discardableResult
    func retry(_ operationId: String, navigation: NavigationPathState?) -> Task<Void, Never>? {
        guard result?.operationId == operationId else { return nil }
        return financial(intent: .retryPublicFunding(operationId: operationId), navigation: navigation, operation: { try await $0.retryFunding(operationId: operationId, action: $1) })
    }

    func retrySync(_ operationId: String) {
        guard result?.operationId == operationId, !cancelled, !isWorking, readTask == nil else { return }
        let current = revision
        readTask = Task { [weak self] in
            guard let self else { return }
            defer {
                if current == revision {
                    readTask = nil
                    if Task.isCancelled, recoveryState.isLoading {
                        recoveryState = .noData
                    }
                }
            }
            guard !cancelled, !Task.isCancelled, current == revision else { return }
            do {
                let saved = try await activeService().retryFundingSync(operationId: operationId)
                guard !cancelled, !Task.isCancelled, current == revision else { return }
                result = saved
                syncFailed = saved.status == .syncFailed
                try await syncPayer(saved.payerWalletId)
                guard !cancelled, !Task.isCancelled, current == revision else { return }
                let snapshot = try await recoverySnapshot(activeService())
                guard !cancelled, !Task.isCancelled, current == revision else { return }
                recoveryState = .data(snapshot)
            } catch {
                guard !cancelled, !Task.isCancelled, current == revision else { return }
                syncFailed = true
                errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
            }
        }
    }

    func discardUnsubmitted(_ operationId: String) {
        guard !cancelled, !isWorking, readTask == nil,
              recoveryState.value?.unsubmitted.contains(where: { $0.operationId == operationId }) == true else { return }
        let current = revision
        readTask = Task { [weak self] in
            guard let self else { return }
            defer {
                if current == revision {
                    readTask = nil
                    if Task.isCancelled, recoveryState.isLoading {
                        recoveryState = .noData
                    }
                }
            }
            guard !cancelled, !Task.isCancelled, current == revision else { return }
            do {
                let service = try activeService()
                try await service.discardUnsubmittedFunding(operationId: operationId)
                guard !cancelled, !Task.isCancelled, current == revision else { return }
                let snapshot = try await recoverySnapshot(service)
                guard !cancelled, !Task.isCancelled, current == revision else { return }
                recoveryState = .data(snapshot)
            } catch {
                guard !cancelled, !Task.isCancelled, current == revision else { return }
                errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
            }
        }
    }

    private func recoverySnapshot(_ service: any BsvHomeBsvMultisigPublicFundingServiceProtocol) async throws -> BsvMultisigFundingRecovery {
        let results = try await service.fundingResults()
        try Task.checkCancellation()
        let unsubmitted = try await service.unsubmittedFunding()
        try Task.checkCancellation()
        return BsvMultisigFundingRecovery(results: results, unsubmitted: unsubmitted)
    }

    public func cancelActions() {
        revision &+= 1
        readTask?.cancel()
        readTask = nil
        if recoveryState.isLoading {
            recoveryState = .noData
        }
        if let action = actionLifetime.cancel(), let scope = action.scope {
            recoveryState = .noData
            do { outcome = try scope.outcome() }
            catch { errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error) }
        }
        isWorking = false
        hasReviewed = false
        preparation = nil
        preparedAt = nil
    }

    public func cancel() {
        cancelActions()
        cancelled = true
        service?.cancel()
    }

    private func activeService() throws -> any BsvHomeBsvMultisigPublicFundingServiceProtocol {
        guard !cancelled else { throw BsvHomeBsvMultisigError.Cancelled }
        if let service {
            return service
        }
        let service = try makeService()
        self.service = service
        return service
    }

    private func financial(
        intent: BsvHomeBsvMultisigActionIntent,
        navigation: NavigationPathState?,
        operation: @escaping @MainActor (any BsvHomeBsvMultisigPublicFundingServiceProtocol, BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigFundingResult,
        onComplete: @escaping @MainActor (BsvHomeBsvMultisigFundingResult) -> Void = { _ in },
    ) -> Task<Void, Never>? {
        guard !cancelled, !Task.isCancelled, !isWorking, readTask == nil else { return nil }
        do {
            let service = try activeService()
            let scope = try service.beginAction(intent: intent)
            hasReviewed = false
            preparation = nil
            preparedAt = nil
            return start(scope: scope, navigation: navigation) { [weak self] in
                let saved = try await operation(service, scope)
                guard let self, !Task.isCancelled else { return }
                result = saved
                syncFailed = saved.status == .syncFailed
                do { outcome = try scope.outcome(); syncFailed = syncFailed || outcome?.syncFailed == true }
                catch { syncFailed = true; errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error) }
                if saved.status != .pending {
                    do { try await syncPayer(saved.payerWalletId) }
                    catch {
                        guard !Task.isCancelled else { return }
                        syncFailed = true
                        errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
                    }
                }
                guard !Task.isCancelled else { return }
                do {
                    let snapshot = try await recoverySnapshot(service)
                    guard !Task.isCancelled else { return }
                    recoveryState = .data(snapshot)
                } catch {
                    guard !Task.isCancelled else { return }
                    syncFailed = true
                    errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
                }
            } onComplete: { [weak self] in
                if let saved = self?.result {
                    onComplete(saved)
                }
            }
        } catch { errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error); return nil }
    }

    private func start(scope: BsvHomeBsvMultisigActionScope?, navigation: NavigationPathState?, operation: @escaping @MainActor () async throws -> Void, onComplete: @escaping @MainActor () -> Void = {}) -> Task<Void, Never> {
        let ticket = actionLifetime.begin(scope: scope, navigation: navigation) { [weak self] in self?.cancelActions() }
        isWorking = true
        errorMessage = nil
        outcome = nil
        syncFailed = false
        let task = Task { [weak self] in
            guard let self else { scope?.cancel(); return }
            await withTaskCancellationHandler {
                guard actionLifetime.contains(ticket), !cancelled, !Task.isCancelled else { return }
                defer {
                    if actionLifetime.contains(ticket), Task.isCancelled {
                        cancelActions()
                    } else if actionLifetime.finish(ticket) {
                        isWorking = false
                    }
                }
                do {
                    try scope?.check()
                    try await operation()
                    guard actionLifetime.contains(ticket), !cancelled, !Task.isCancelled else { return }
                    guard actionLifetime.finish(ticket) else { return }
                    isWorking = false
                    onComplete()
                } catch {
                    guard actionLifetime.contains(ticket), !cancelled, !Task.isCancelled else { return }
                    errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error)
                    if let scope {
                        recoveryState = .noData
                        do { outcome = try scope.outcome(); syncFailed = outcome?.syncFailed == true }
                        catch { errorMessage = BsvHomeBsvMultisigError.displayedMessage(for: error) }
                    }
                }
            } onCancel: {
                scope?.cancel()
                Task { @MainActor [weak self] in
                    guard self?.actionLifetime.contains(ticket) == true else { return }
                    self?.cancelActions()
                }
            }
        }
        actionLifetime.attach({ task.cancel() }, to: ticket)
        return task
    }
}
