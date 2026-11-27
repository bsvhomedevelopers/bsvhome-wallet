// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import WebSocketClient

public actor WebSocketConnectionMock: WebSocketConnectable {
    private var continuation: AsyncStream<WebSocketEvent>.Continuation?
    private var sentData: [Data] = []
    private let onConnect: @Sendable () -> Void
    private let onSend: (@Sendable (Data) -> Void)?
    private let onStateRead: (@Sendable (WebSocketState) -> Void)?
    private var connectionState: WebSocketState = .disconnected

    public private(set) var state: WebSocketState {
        get {
            onStateRead?(connectionState)
            return connectionState
        }
        set { connectionState = newValue }
    }

    public init(
        onConnect: @escaping @Sendable () -> Void = {},
        onSend: (@Sendable (Data) -> Void)? = nil,
        onStateRead: (@Sendable (WebSocketState) -> Void)? = nil,
    ) {
        self.onConnect = onConnect
        self.onSend = onSend
        self.onStateRead = onStateRead
    }

    // MARK: - WebSocketConnectable

    public func connect() -> AsyncStream<WebSocketEvent> {
        let (stream, continuation) = AsyncStream<WebSocketEvent>.makeStream()
        self.continuation = continuation
        state = .connecting
        onConnect()
        return stream
    }

    public func disconnect() async {
        state = .disconnected
        continuation?.yield(.disconnected(nil))
        continuation?.finish()
        continuation = nil
    }

    public func send(_ data: Data) async throws {
        guard state == .connected else {
            throw WebSocketError.notConnected
        }
        sentData.append(data)
        onSend?(data)
    }

    public func send(_ text: String) async throws {
        try await send(Data(text.utf8))
    }

    // MARK: - Mock Control API

    public func simulateConnected() {
        state = .connected
        continuation?.yield(.connected)
    }

    public func simulateMessage(_ data: Data) {
        continuation?.yield(.message(data))
    }

    public func simulateDisconnect(error: Error? = nil) {
        state = .disconnected
        continuation?.yield(.disconnected(error))
    }

    public func getSentData() -> [Data] {
        sentData
    }

    public func clearSentData() {
        sentData.removeAll()
    }
}
