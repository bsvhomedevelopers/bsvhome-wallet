import Foundation
import protocol BsvHomeCore.BsvHomeBsvReceiptServiceProtocol

final class BsvReceiptCoreServiceMock: BsvHomeBsvReceiptServiceProtocol, Sendable {
    private let prepare: @Sendable () async throws -> Bool
    private let onConnected: @Sendable () async throws -> String
    private let onRefresh: @Sendable () async throws -> String?
    private let onEvent: @Sendable (String) throws -> Void
    private let onRun: (@Sendable () async throws -> Void)?
    private let onCancel: @Sendable () -> Void
    private let cancelled: AsyncStream<Void>
    private let cancellation: AsyncStream<Void>.Continuation

    init(
        prepare: @escaping @Sendable () async throws -> Bool = { true },
        onConnected: @escaping @Sendable () async throws -> String = { "subscription" },
        onRefresh: @escaping @Sendable () async throws -> String? = { nil },
        onEvent: @escaping @Sendable (String) throws -> Void = { _ in },
        onRun: (@Sendable () async throws -> Void)? = nil,
        onCancel: @escaping @Sendable () -> Void = {},
    ) {
        self.prepare = prepare
        self.onConnected = onConnected
        self.onRefresh = onRefresh
        self.onEvent = onEvent
        self.onRun = onRun
        self.onCancel = onCancel
        let stream = AsyncStream<Void>.makeStream()
        cancelled = stream.stream
        cancellation = stream.continuation
    }

    func url() -> String { "wss://receipt.test/wallet-events" }
    func prepareConnection() async throws -> Bool { try await prepare() }
    func connected() async throws -> String { try await onConnected() }
    func refreshSubscription() async throws -> String? { try await onRefresh() }
    func receiveEvent(event: String) throws { try onEvent(event) }

    func run() async throws {
        if let onRun {
            try await onRun()
        } else {
            for await _ in cancelled {}
            try Task.checkCancellation()
        }
    }

    func cancel() {
        cancellation.finish()
        onCancel()
    }
}
