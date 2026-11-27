import WebSocketClient

struct BsvReceiptReconnectMock: Reconnectable {
    var delay: @Sendable (UInt32) -> UInt64 = { _ in 0 }

    func reconnectDelayMilliseconds(attempt: UInt32) -> UInt64 {
        delay(attempt)
    }

    func pingIntervalMilliseconds() -> UInt64 {
        30000
    }
}
