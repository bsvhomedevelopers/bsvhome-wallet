import CoreFoundation
import Foundation
import Testing
import WebSocketClient

struct BsvReceiptWebSocketLiveTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["BSV_RECEIPT_WEBSOCKET_LIVE_TEST"] == "1"), .timeLimit(.minutes(1)))
    func publicBurnAddressSubscriptionPassesNativeTls() async throws {
        let fixture = (
            address: "1BitcoinEaterAddressDontSendf59kuE",
            scriptHash: "5f923a2ae63c4de79d21fe806f3686850af2e61b99a174119624073139f394c2",
        )
        let url = try #require(URL(string: "wss://paymail.bsvhome.com/api/v1/wallet-events"))
        let session = URLSessionConfiguration.ephemeral
        session.httpShouldSetCookies = false
        session.httpCookieStorage = nil
        session.urlCredentialStorage = nil
        session.urlCache = nil
        session.requestCachePolicy = .reloadIgnoringLocalCacheData
        session.timeoutIntervalForRequest = 20
        session.timeoutIntervalForResource = 25
        let connection = WebSocketConnection(configuration: WebSocketConfiguration(
            request: URLRequest(url: url),
            reconnection: LiveReconnection(),
            sessionConfiguration: session,
            redirectPolicy: .reject,
            receiveBufferLimit: 32,
            maximumMessageSize: 150_000,
            queueMessagesWhileConnecting: false,
        ))
        let subscription = Subscription(
            requestId: UUID().uuidString.lowercased(),
            scriptHashes: [fixture.scriptHash],
        )
        let payload = try JSONEncoder().encode(subscription)
        do {
            let summary = try await sendSubscription(connection: connection, payload: String(decoding: payload, as: UTF8.self))
            await connection.disconnect()
            print("BSV receipt WebSocket connected for public burn address \(fixture.address); subscription send completed. No acknowledgement or receipt event is asserted.")
            if let summary {
                print("Received frame fields/types: \(summary)")
            }
        } catch {
            await connection.disconnect()
            throw error
        }
    }

    private func sendSubscription(connection: WebSocketConnection, payload: String) async throws -> String? {
        let progress = SubscriptionProgress()
        let events = await connection.connect()
        return try await withThrowingTaskGroup(of: String?.self) { group in
            group.addTask {
                var sent = false
                for await event in events {
                    switch event {
                    case .connected:
                        try await connection.send(payload)
                        sent = true
                        await progress.markSent()
                    case let .message(data):
                        if sent {
                            return Self.frameSummary(data)
                        }
                    case let .disconnected(error):
                        if sent {
                            return nil
                        }
                        throw error ?? ProbeFailure.connectionEnded
                    }
                }
                throw ProbeFailure.connectionEnded
            }
            group.addTask {
                try await Task.sleep(for: .seconds(20))
                guard await progress.sent else { throw ProbeFailure.timedOut }
                return nil
            }
            defer { group.cancelAll() }
            guard let summary = try await group.next() else { throw ProbeFailure.connectionEnded }
            return summary
        }
    }

    private static func frameSummary(_ data: Data) -> String {
        guard let value = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) else { return "non-JSON" }
        guard let object = value as? [String: Any] else { return fieldType(value) }
        return object.sorted { $0.key < $1.key }.prefix(20).map { key, value in
            let name = key.count <= 64 && key.unicodeScalars.allSatisfy { CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "_.-")).contains($0) } ? key : "redacted-key"
            return "\(name):\(fieldType(value))"
        }.joined(separator: ", ")
    }

    private static func fieldType(_ value: Any) -> String {
        switch value {
        case is NSNull: "null"
        case is String: "string"
        case is [Any]: "array"
        case is [String: Any]: "object"
        case let value as NSNumber: CFGetTypeID(value) == CFBooleanGetTypeID() ? "boolean" : "number"
        default: "other"
        }
    }

    private struct Subscription: Encodable, Sendable {
        let version = 1
        let type = "subscribe"
        let network = "bsv-mainnet"
        let requestId: String
        let scriptHashes: [String]
    }

    private struct LiveReconnection: Reconnectable {
        func reconnectDelayMilliseconds(attempt _: UInt32) -> UInt64 {
            60000
        }

        func pingIntervalMilliseconds() -> UInt64 {
            60000
        }
    }

    private actor SubscriptionProgress {
        private(set) var sent = false
        func markSent() {
            sent = true
        }
    }

    private enum ProbeFailure: Error {
        case connectionEnded
        case timedOut
    }
}
