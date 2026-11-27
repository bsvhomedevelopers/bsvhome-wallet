// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeBalanceStore
import struct BsvHomeCore.BsvHomeAssetBalance
import struct BsvHomeCore.BsvHomeBalanceRecord
import struct BsvHomeCore.BsvHomeBalanceValue
import typealias BsvHomeCore.AssetId
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreBalanceStore: BsvHomeBalanceStore, @unchecked Sendable {
    private let store: BalanceStore

    public init(store: BalanceStore) {
        self.store = store
    }

    public func getAvailableBalances(walletId: String, assetIds: [BsvHomeCore.AssetId]) throws -> [BsvHomeAssetBalance] {
        try store.getBalances(walletId: WalletId.from(id: walletId), assetIds: assetIds.map { try Primitives.AssetId(id: $0) })
            .map { BsvHomeAssetBalance($0.balance, assetId: $0.assetId, isActive: $0.isActive) }
    }

    public func updateBalances(walletId: String, balances: [BsvHomeBalanceRecord]) async throws {
        let walletId = try WalletId.from(id: walletId)
        let updates = try balances.map { balance in
            try UpdateBalance(
                assetId: Primitives.AssetId(id: balance.assetId),
                available: value(balance.available),
                frozen: value(balance.frozen),
                locked: value(balance.locked),
                staked: value(balance.staked),
                pending: value(balance.pending),
                pendingUnconfirmed: value(balance.pendingUnconfirmed),
                rewards: value(balance.rewards),
                reserved: value(balance.reserved),
                withdrawable: value(balance.withdrawable),
                earn: value(balance.earn),
                metadata: balance.metadata.map { $0.map() },
                updatedAt: .now,
                isActive: balance.isActive,
            )
        }
        try store.updateBalances(updates, for: walletId)
    }

    public func getEnabledAssetIds(walletId: String) async throws -> [BsvHomeCore.AssetId] {
        try store.getEnabledAssetIds(walletId: WalletId.from(id: walletId)).map(\Primitives.AssetId.identifier)
    }

    public func setAssetsEnabled(walletId: String, assetIds: [BsvHomeCore.AssetId], enabled: Bool) async throws {
        try store.setIsEnabled(walletId: WalletId.from(id: walletId), assetIds: assetIds.map { try Primitives.AssetId(id: $0) }, value: enabled)
    }

    public func setAssetPinned(walletId: String, assetId: BsvHomeCore.AssetId, pinned: Bool) async throws {
        try store.pinAsset(walletId: WalletId.from(id: walletId), assetId: Primitives.AssetId(id: assetId), value: pinned)
    }

    private func value(_ value: BsvHomeBalanceValue) -> UpdateBalanceValue {
        UpdateBalanceValue(value: value.value.description, amount: value.amount)
    }
}
