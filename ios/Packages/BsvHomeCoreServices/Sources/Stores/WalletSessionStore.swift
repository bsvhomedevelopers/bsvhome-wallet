// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import Observation
import protocol BsvHomeCore.BsvHomePreferencesStore
import protocol BsvHomeCore.BsvHomeWalletSessionStore
import typealias BsvHomeCore.WalletId

@Observable
public final class BsvHomeCoreWalletSessionStore: BsvHomeWalletSessionStore, @unchecked Sendable {
    private static let key = "current_wallet_id"

    private let store: any BsvHomePreferencesStore

    public init(store: any BsvHomePreferencesStore) {
        self.store = store
    }

    @ObservationIgnored
    public var currentWalletId: BsvHomeCore.WalletId? {
        access(keyPath: \.currentWalletId)
        return store.get(key: Self.key)
    }

    public func getCurrentWalletId() throws -> BsvHomeCore.WalletId? {
        currentWalletId
    }

    public func setCurrentWalletId(walletId: BsvHomeCore.WalletId?) throws {
        switch walletId {
        case let .some(walletId): try store.set(key: Self.key, value: walletId)
        case .none: try store.remove(key: Self.key)
        }
        withMutation(keyPath: \.currentWalletId) {}
    }
}
