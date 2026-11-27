import Foundation
import Keychain

public actor WowPayMpcParticipant {
    private let connection: WowPayMpcConnection

    private init(connection: WowPayMpcConnection) {
        self.connection = connection
    }

    public static func open(
        walletId: String,
        namespace: String,
        keychain: any Keychain,
        rootURL: URL? = nil,
    ) async throws -> WowPayMpcParticipant {
        try Task.checkCancellation()
        let connection = WowPayMpcConnection()
        try await connection.open(walletId: walletId, namespace: namespace, keychain: keychain, rootURL: rootURL)
        do {
            try Task.checkCancellation()
        } catch {
            try await connection.close()
            throw error
        }
        return WowPayMpcParticipant(connection: connection)
    }

    public func exchange(_ commandJSON: String) async throws -> String {
        try Task.checkCancellation()
        let result = try await connection.exchange(commandJSON)
        try Task.checkCancellation()
        return result
    }

    public func close() async throws {
        try await connection.close()
    }
}
