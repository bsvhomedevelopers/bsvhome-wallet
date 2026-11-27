import Foundation
import BsvHomeCore

final class BsvMultisigPublicFundingServiceMock: BsvHomeBsvMultisigPublicFundingServiceProtocol, @unchecked Sendable {
    var onResults: @Sendable () async throws -> [BsvHomeBsvMultisigFundingResult] = { [] }
    var onUnsubmitted: @Sendable () async throws -> [BsvHomeBsvMultisigUnsubmittedFunding] = { [] }
    var onPrepare: (@Sendable (BsvHomeBsvMultisigPublicFundingSource, String) async throws -> BsvHomeBsvMultisigFundingPreparation)?
    var onSync: (@Sendable (String) async throws -> BsvHomeBsvMultisigFundingResult)?
    var onDiscard: (@Sendable (String) async throws -> Void)?
    var onSubmit: (@Sendable (String, BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigFundingResult)?
    var onRetry: (@Sendable (String, BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigFundingResult)?
    var actionScopeFactory: (@Sendable () throws -> BsvHomeBsvMultisigActionScope)?
    private let lock = NSLock()
    private var recordedFactoryCreations = 0
    private var recordedCalls: [String] = []
    private var recordedAmounts: [String] = []
    private var recordedOperations: [String] = []
    private var recordedScopes: [BsvHomeBsvMultisigActionScope] = []

    var factoryCreationCount: Int { lock.withLock { recordedFactoryCreations } }
    var calls: [String] { lock.withLock { recordedCalls } }
    var amounts: [String] { lock.withLock { recordedAmounts } }
    var operations: [String] { lock.withLock { recordedOperations } }
    var scopes: [BsvHomeBsvMultisigActionScope] { lock.withLock { recordedScopes } }

    func recordFactoryCreation() {
        lock.withLock { recordedFactoryCreations += 1 }
    }

    func fundingResults() async throws -> [BsvHomeBsvMultisigFundingResult] {
        record("results")
        return try await onResults()
    }

    func fundingResult(operationId: String) async throws -> BsvHomeBsvMultisigFundingResult? {
        record("result")
        return try await onResults().first { $0.operationId == operationId }
    }

    func unsubmittedFunding() async throws -> [BsvHomeBsvMultisigUnsubmittedFunding] {
        record("unsubmitted")
        return try await onUnsubmitted()
    }

    func prepareFunding(source: BsvHomeBsvMultisigPublicFundingSource, amountBsv: String) async throws -> BsvHomeBsvMultisigFundingPreparation {
        record("prepare")
        lock.withLock { recordedAmounts.append(amountBsv) }
        guard let onPrepare else { throw BsvHomeBsvMultisigError.MissingRecord }
        return try await onPrepare(source, amountBsv)
    }

    func beginAction(intent _: BsvHomeBsvMultisigActionIntent) throws -> BsvHomeBsvMultisigActionScope {
        record("begin")
        guard let actionScopeFactory else { throw BsvHomeBsvMultisigError.MissingRecord }
        let scope = try actionScopeFactory()
        lock.withLock { recordedScopes.append(scope) }
        return scope
    }

    func submitFunding(preparationId: String, action: BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigFundingResult {
        record("submit")
        lock.withLock { recordedOperations.append(preparationId) }
        guard let onSubmit else { throw BsvHomeBsvMultisigError.MissingRecord }
        return try await onSubmit(preparationId, action)
    }

    func retryFunding(operationId: String, action: BsvHomeBsvMultisigActionScope) async throws -> BsvHomeBsvMultisigFundingResult {
        record("retry")
        lock.withLock { recordedOperations.append(operationId) }
        guard let onRetry else { throw BsvHomeBsvMultisigError.MissingRecord }
        return try await onRetry(operationId, action)
    }

    func retryFundingSync(operationId: String) async throws -> BsvHomeBsvMultisigFundingResult {
        record("sync")
        lock.withLock { recordedOperations.append(operationId) }
        guard let onSync else { throw BsvHomeBsvMultisigError.MissingRecord }
        return try await onSync(operationId)
    }

    func discardUnsubmittedFunding(operationId: String) async throws {
        record("discard")
        lock.withLock { recordedOperations.append(operationId) }
        guard let onDiscard else { throw BsvHomeBsvMultisigError.MissingRecord }
        try await onDiscard(operationId)
    }

    func cancel() {
        record("cancel")
    }

    private func record(_ value: String) {
        lock.withLock { recordedCalls.append(value) }
    }
}
