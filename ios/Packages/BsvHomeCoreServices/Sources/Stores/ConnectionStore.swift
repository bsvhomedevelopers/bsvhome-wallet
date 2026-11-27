// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeConnectionStore
import struct BsvHomeCore.WalletConnection
import struct BsvHomeCore.WalletConnectionSession
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreConnectionStore: BsvHomeConnectionStore, @unchecked Sendable {
    private let store: ConnectionStore

    public init(store: ConnectionStore) {
        self.store = store
    }

    public func getConnection(sessionId: String) async throws -> BsvHomeCore.WalletConnection? {
        try store.getConnection(sessionId: sessionId).map { $0.map() }
    }

    public func getSessions() async throws -> [BsvHomeCore.WalletConnectionSession] {
        try store.getSessions().map { $0.map() }
    }

    public func addConnection(connection: BsvHomeCore.WalletConnection) async throws {
        try store.addConnection(connection.map())
    }

    public func updateSession(session: BsvHomeCore.WalletConnectionSession) async throws {
        try store.updateConnectionSession(session.map())
    }

    public func deleteSessions(sessionIds: [String]) async throws {
        _ = try store.delete(ids: sessionIds)
    }
}
