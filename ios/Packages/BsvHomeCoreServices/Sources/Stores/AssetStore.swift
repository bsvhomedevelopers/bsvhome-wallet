// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import typealias BsvHomeCore.Asset
import typealias BsvHomeCore.AssetBasic
import typealias BsvHomeCore.AssetFull
import typealias BsvHomeCore.AssetId
import protocol BsvHomeCore.BsvHomeAssetStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreAssetStore: BsvHomeAssetStore, @unchecked Sendable {
    private let assetStore: AssetStore
    private let balanceStore: BalanceStore

    public init(assetStore: AssetStore, balanceStore: BalanceStore) {
        self.assetStore = assetStore
        self.balanceStore = balanceStore
    }

    public func getAssetIds(assetIds: [BsvHomeCore.AssetId]) async throws -> [BsvHomeCore.AssetId] {
        try assetStore.getAssets(for: assetIds).map(\.id.identifier)
    }

    public func getAssets(assetIds: [BsvHomeCore.AssetId]) throws -> [BsvHomeCore.Asset] {
        try assetStore.getAssets(for: assetIds).map { $0.map() }
    }

    public func saveAssets(assets: [BsvHomeCore.AssetBasic]) async throws {
        try assetStore.add(assets: assets.map { $0.map() })
    }

    public func saveAsset(asset: BsvHomeCore.AssetFull) async throws {
        let asset = asset.map()
        try assetStore.add(assets: [asset.basic])
        try assetStore.updateLinks(assetId: asset.asset.id, asset.links)
        try assetStore.updateAssociations(assetId: asset.asset.id, associations: asset.associations)
    }

    public func setBuyableAssets(assetIds: [BsvHomeCore.AssetId]) async throws {
        try assetStore.updateBuyableAssets(assetIds: assetIds)
    }

    public func setSellableAssets(assetIds: [BsvHomeCore.AssetId]) async throws {
        try assetStore.updateSellableAssets(assetIds: assetIds)
    }

    public func setStakeableAssets(assetIds: [BsvHomeCore.AssetId]) async throws {
        try assetStore.setAssetIsStakeable(for: assetIds, value: true)
    }

    public func setSwappableAssets(assetIds: [BsvHomeCore.AssetId]) async throws {
        try assetStore.setAssetIsSwappable(for: assetIds, value: true)
    }

    public func addBalances(walletId: String, assetIds: [BsvHomeCore.AssetId], enabled: Bool) async throws {
        try balanceStore.addBalance(
            assetIds: assetIds.map { try Primitives.AssetId(id: $0) },
            isEnabled: enabled,
            for: WalletId.from(id: walletId),
        )
    }

    public func addMissingBalances(walletId: String, assetIds: [BsvHomeCore.AssetId]) async throws {
        try balanceStore.addBalance(
            assetIds: assetIds.map { try Primitives.AssetId(id: $0) },
            isEnabled: false,
            for: WalletId.from(id: walletId),
        )
    }
}
