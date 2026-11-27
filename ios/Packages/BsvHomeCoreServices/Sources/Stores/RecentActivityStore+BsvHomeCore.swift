// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import BsvHomeCorePrimitives
import Primitives
import Store
import struct BsvHomeCore.BsvHomeRecentActivity
import protocol BsvHomeCore.BsvHomeRecentActivityStore
import enum BsvHomeCore.RecentActivityType
import typealias BsvHomeCore.WalletId

public final class BsvHomeCoreRecentActivityStore: BsvHomeRecentActivityStore, @unchecked Sendable {
    private let store: RecentActivityStore

    public init(store: RecentActivityStore) {
        self.store = store
    }

    public func add(activity: BsvHomeRecentActivity, walletId: BsvHomeCore.WalletId) async throws {
        try store.add(RecentActivityData(activity), walletId: Primitives.WalletId.from(id: walletId))
    }

    public func clear(walletId: BsvHomeCore.WalletId, types: [BsvHomeCore.RecentActivityType]) async throws {
        try store.clear(walletId: Primitives.WalletId.from(id: walletId), types: types.map { $0.map() })
    }
}
