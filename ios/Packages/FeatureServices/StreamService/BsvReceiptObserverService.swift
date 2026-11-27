import Foundation
import protocol BsvHomeCore.BsvHomeBsvReceiptServiceProtocol
import BsvHomeCorePrimitives
import Primitives
import WebSocketClient

public actor BsvReceiptObserverService: Sendable {
    private enum Event: Sendable {
        case addressesChanged
        case socket(WebSocketEvent)
    }

    private let makeService: @Sendable () throws -> any BsvHomeBsvReceiptServiceProtocol
    private let makeWebSocket: @Sendable (String) throws -> any WebSocketConnectable
    private let reconnection: any Reconnectable
    private let addressChanges: @Sendable () -> AsyncThrowingStream<Void, any Error>
    private var observeTask: Task<Void, Never>?
    private var isActive = false
    private var reconnectAttempt: UInt32 = 0
    private var service: (any BsvHomeBsvReceiptServiceProtocol)?
    private var webSocket: (any WebSocketConnectable)?

    public init(
        makeService: @escaping @Sendable () throws -> any BsvHomeBsvReceiptServiceProtocol,
        makeWebSocket: @escaping @Sendable (String) throws -> any WebSocketConnectable,
        reconnection: any Reconnectable,
        addressChanges: @escaping @Sendable () -> AsyncThrowingStream<Void, any Error>,
    ) {
        self.makeService = makeService
        self.makeWebSocket = makeWebSocket
        self.reconnection = reconnection
        self.addressChanges = addressChanges
    }

    deinit {
        observeTask?.cancel()
    }

    public func connect() {
        guard !isActive else { return }
        isActive = true
        restart()
    }

    public func update() {
        guard isActive else { return }
        restart()
    }

    public func disconnect() async {
        isActive = false
        observeTask?.cancel()
        service?.cancel()
        await observeTask?.value
    }

    private func restart() {
        let previous = observeTask
        previous?.cancel()
        observeTask = Task { [weak self] in
            await previous?.value
            guard !Task.isCancelled, let self else { return }
            await observeConnection()
        }
    }

    private func observeConnection() async {
        while !Task.isCancelled {
            do {
                let service = try makeService()
                let webSocket = try makeWebSocket(service.url())
                self.service = service
                self.webSocket = webSocket
                guard try await prepareConnection(service: service) else { break }
                try Task.checkCancellation()
                try await observePreparedConnection(service: service, webSocket: webSocket)
            } catch is CancellationError {
                break
            } catch {
                guard !Task.isCancelled else { break }
                debugLog("BSV receipt observer failed; reconnecting")
            }
            service?.cancel()
            await webSocket?.disconnect()
            guard !Task.isCancelled else { break }
            do {
                try await Task.sleep(for: .milliseconds(reconnection.reconnectDelayMilliseconds(attempt: reconnectAttempt)))
            } catch {
                break
            }
            if reconnectAttempt < UInt32.max {
                reconnectAttempt += 1
            }
        }
        service?.cancel()
        await webSocket?.disconnect()
        service = nil
        webSocket = nil
    }

    private func prepareConnection(service: any BsvHomeBsvReceiptServiceProtocol) async throws -> Bool {
        if try await service.prepareConnection() {
            return true
        }
        for try await _ in addressChanges() {
            try Task.checkCancellation()
            if try await service.prepareConnection() {
                return true
            }
        }
        return false
    }

    private func observePreparedConnection(service: any BsvHomeBsvReceiptServiceProtocol, webSocket: any WebSocketConnectable) async throws {
        let addressChanges = addressChanges
        let events = AsyncThrowingStream<Event, any Error>.makeStream(bufferingPolicy: .bufferingNewest(32))
        try await withThrowingTaskGroup(of: Void.self) { group in
            group.addTask { try await service.run() }
            group.addTask {
                do {
                    for try await _ in addressChanges() {
                        try Task.checkCancellation()
                        try Self.enqueue(.addressesChanged, continuation: events.continuation)
                    }
                    events.continuation.finish()
                } catch {
                    events.continuation.finish(throwing: error)
                }
            }
            group.addTask {
                for await event in await webSocket.connect() {
                    try Task.checkCancellation()
                    try Self.enqueue(.socket(event), continuation: events.continuation)
                }
                events.continuation.finish()
            }
            group.addTask {
                for try await event in events.stream {
                    try Task.checkCancellation()
                    try await self.receive(event, service: service, webSocket: webSocket)
                }
            }
            do {
                _ = try await group.next()
            } catch {
                service.cancel()
                await webSocket.disconnect()
                group.cancelAll()
                throw error
            }
            service.cancel()
            await webSocket.disconnect()
            group.cancelAll()
        }
    }

    private nonisolated static func enqueue(_ event: Event, continuation: AsyncThrowingStream<Event, any Error>.Continuation) throws {
        if case .dropped = continuation.yield(event) {
            throw AnyError("BSV receipt event queue overflow")
        }
    }

    private func receive(_ event: Event, service: any BsvHomeBsvReceiptServiceProtocol, webSocket: any WebSocketConnectable) async throws {
        switch event {
        case .addressesChanged:
            if let message = try await service.refreshSubscription(), await webSocket.state == .connected {
                try Task.checkCancellation()
                try await webSocket.send(message)
            }
        case .socket(.connected):
            reconnectAttempt = 0
            let message = try await service.connected()
            try Task.checkCancellation()
            try await webSocket.send(message)
        case let .socket(.message(data)):
            guard let event = String(data: data, encoding: .utf8) else { return }
            try service.receiveEvent(event: event)
        case .socket(.disconnected):
            break
        }
    }
}
