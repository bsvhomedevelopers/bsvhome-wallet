import struct BsvHomeCore.BsvHomePaymailBindingIntent
import enum BsvHomeCore.BsvHomePaymailError
import struct BsvHomeCore.BsvHomePaymailMutation
import protocol BsvHomeCore.BsvHomePaymailServiceProtocol
import struct BsvHomeCore.BsvHomePaymailSnapshot

public actor BsvHomePaymailServiceMock: BsvHomePaymailServiceProtocol {
    private var snapshot: BsvHomePaymailSnapshot
    private let mutation: BsvHomePaymailMutation
    private let onLoad: @Sendable () async throws -> Void
    private var loadError: BsvHomePaymailError?
    public private(set) var intents: [BsvHomePaymailBindingIntent] = []
    public private(set) var submissionCount = 0

    public init(snapshot: BsvHomePaymailSnapshot, mutation: BsvHomePaymailMutation, onLoad: @escaping @Sendable () async throws -> Void = {}) {
        self.snapshot = snapshot
        self.mutation = mutation
        self.onLoad = onLoad
    }

    public func setLoadError(_ error: BsvHomePaymailError) {
        loadError = error
    }

    public func setSnapshot(_ snapshot: BsvHomePaymailSnapshot) {
        self.snapshot = snapshot
        loadError = nil
    }

    public func load() async throws -> BsvHomePaymailSnapshot {
        try await onLoad()
        if let loadError {
            throw loadError
        }
        return snapshot
    }

    public func bindingIntent(snapshot: BsvHomePaymailSnapshot, alias: String, domain: String, active: Bool) async throws -> BsvHomePaymailBindingIntent {
        let intent = BsvHomePaymailBindingIntent(walletId: snapshot.walletId, address: snapshot.address, publicOrigin: snapshot.publicOrigin, handle: "\(alias)@\(domain)", active: active, previousVersion: 0)
        intents.append(intent)
        return intent
    }

    public func submitBinding(intent _: BsvHomePaymailBindingIntent) async throws -> BsvHomePaymailMutation {
        submissionCount += 1
        return mutation
    }

    public nonisolated func cancel() {}
}
