import Foundation
import Testing
@testable import WebSocketClient

@Suite(.timeLimit(.minutes(1)))
struct WebSocketReceiveLimitTests {
    @Test
    func overflowEndsTheStreamWithoutReconnectingInPlace() async {
        let connection = connection(receiveBufferLimit: 2)
        let events = await connection.connect()
        var iterator = events.makeAsyncIterator()
        _ = await iterator.next()
        await connection.handleMessage(.string("first"))
        await connection.handleMessage(.string("second"))
        await connection.handleMessage(.string("overflow"))
        #expect(await connection.state == .disconnected)
        await connection.handleMessage(.string("late"))
        await connection.disconnect()
        let messages = await messages(from: &iterator)
        #expect(messages == ["first", "second"])
    }

    @Test
    func maximumFrameSizeCountsUtf8BytesAndClosesTheStream() async {
        let connection = connection(maximumMessageSize: 4)
        let events = await connection.connect()
        var iterator = events.makeAsyncIterator()
        _ = await iterator.next()
        await connection.handleMessage(.data(Data([1, 2, 3, 4])))
        await connection.handleMessage(.string("ééé"))
        #expect(await connection.state == .disconnected)
        await connection.disconnect()
        var received: [Data] = []
        while let event = await iterator.next() {
            if case let .message(data) = event {
                received.append(data)
            }
        }
        #expect(received == [Data([1, 2, 3, 4])])
    }

    @Test
    func disabledOfflineQueueRejectsSubscriptionMessages() async {
        let connection = connection(queueMessagesWhileConnecting: false)
        let events = await connection.connect()
        var iterator = events.makeAsyncIterator()
        _ = await iterator.next()
        #expect(await connection.state == .reconnecting)
        await #expect(throws: WebSocketError.self) { try await connection.send("stale-subscription") }
        await #expect(throws: WebSocketError.self) { try await connection.send(Data("stale-subscription".utf8)) }
        await connection.disconnect()
        await #expect(throws: WebSocketError.self) { try await connection.send("after-stop") }
        _ = await iterator.next()
    }

    @Test
    func defaultConfigurationKeepsUnboundedFramesAndOfflineQueue() async {
        let connection = connection()
        let events = await connection.connect()
        var iterator = events.makeAsyncIterator()
        _ = await iterator.next()
        await #expect(throws: Never.self) { try await connection.send("queued-message") }
        let large = String(repeating: "x", count: 150_001)
        await connection.handleMessage(.string(large))
        for _ in 0 ..< 40 {
            await connection.handleMessage(.string("message"))
        }
        #expect(await connection.state == .reconnecting)
        await connection.disconnect()
        let received = await messages(from: &iterator)
        #expect(received.count == 41)
        #expect(received.first == large)
    }

    private func connection(
        receiveBufferLimit: Int? = nil,
        maximumMessageSize: Int? = nil,
        queueMessagesWhileConnecting: Bool = true,
    ) -> WebSocketConnection {
        WebSocketConnection(configuration: WebSocketConfiguration(
            requestProvider: FailingRequestProvider(),
            reconnection: DelayedReconnection(),
            receiveBufferLimit: receiveBufferLimit,
            maximumMessageSize: maximumMessageSize,
            queueMessagesWhileConnecting: queueMessagesWhileConnecting,
        ))
    }

    private func messages(from iterator: inout AsyncStream<WebSocketEvent>.Iterator) async -> [String] {
        var values: [String] = []
        while let event = await iterator.next() {
            if case let .message(data) = event {
                values.append(String(decoding: data, as: UTF8.self))
            }
        }
        return values
    }

    private struct FailingRequestProvider: WebSocketRequestProvider {
        func makeRequest() throws -> URLRequest {
            throw WebSocketError.notConnected
        }
    }

    private struct DelayedReconnection: Reconnectable {
        func reconnectDelayMilliseconds(attempt _: UInt32) -> UInt64 {
            60000
        }

        func pingIntervalMilliseconds() -> UInt64 {
            60000
        }
    }
}
