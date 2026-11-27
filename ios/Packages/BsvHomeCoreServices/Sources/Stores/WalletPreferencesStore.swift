// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeServiceError
import protocol BsvHomeCore.BsvHomeWalletPreferencesStore
import typealias BsvHomeCore.WalletId

public final class BsvHomeCoreWalletPreferencesStore: BsvHomeWalletPreferencesStore, Sendable {
    private let namespace: String

    public init(namespace: String = "") {
        self.namespace = namespace
    }

    public func get(walletId: BsvHomeCore.WalletId, key: String) -> String? {
        defaults(walletId: walletId)?.preferenceValue(forKey: key)
    }

    public func set(walletId: BsvHomeCore.WalletId, key: String, value: String) throws {
        guard let defaults = defaults(walletId: walletId) else {
            throw BsvHomeServiceError.Store(msg: "Wallet preferences are unavailable")
        }
        defaults.set(value, forKey: key)
    }

    public func deletePreferences(walletId: BsvHomeCore.WalletId) throws {
        UserDefaults.standard.removePersistentDomain(forName: suiteName(walletId: walletId))
    }

    private func defaults(walletId: BsvHomeCore.WalletId) -> UserDefaults? {
        UserDefaults(suiteName: suiteName(walletId: walletId)) ?? (namespace.isEmpty ? .standard : nil)
    }

    private func suiteName(walletId: BsvHomeCore.WalletId) -> String {
        "\(namespace)wallet_preferences_\(walletId)_v2"
    }
}
