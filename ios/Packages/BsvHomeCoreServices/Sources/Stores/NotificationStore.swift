// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.InAppNotification
import protocol BsvHomeCore.BsvHomeNotificationStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreNotificationStore: BsvHomeNotificationStore, @unchecked Sendable {
    private let store: InAppNotificationStore

    public init(store: InAppNotificationStore) {
        self.store = store
    }

    public func saveNotifications(notifications: [BsvHomeCore.InAppNotification]) async throws {
        try store.addNotifications(notifications.map { $0.map() })
    }

    public func hasUnreadNotifications(walletId: String) async throws -> Bool {
        try store.hasUnreadNotifications(walletId: WalletId.from(id: walletId))
    }
}
