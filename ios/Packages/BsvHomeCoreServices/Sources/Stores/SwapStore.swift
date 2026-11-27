// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import typealias BsvHomeCore.AssetId
import typealias BsvHomeCore.Chain
import struct BsvHomeCore.BsvHomeSwapPair
import protocol BsvHomeCore.BsvHomeSwapStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreSwapStore: BsvHomeSwapStore, @unchecked Sendable {
    private let assetStore: AssetStore
    private let transactionStore: TransactionStore
    private let recentActivityStore: RecentActivityStore

    public init(
        assetStore: AssetStore,
        transactionStore: TransactionStore,
        recentActivityStore: RecentActivityStore,
    ) {
        self.assetStore = assetStore
        self.transactionStore = transactionStore
        self.recentActivityStore = recentActivityStore
    }

    public func getSwapPairs(walletId: String) async throws -> [BsvHomeCore.BsvHomeSwapPair] {
        try transactionStore.getSwapHistory(walletId: WalletId.from(id: walletId))
            .map { BsvHomeSwapPair(fromAssetId: $0.fromAsset.identifier, toAssetId: $0.toAsset.identifier) }
    }

    public func getRecentAssetIds(walletId: String) async throws -> [BsvHomeCore.AssetId] {
        try recentActivityStore.getRecent(walletId: WalletId.from(id: walletId), types: [.swapSelect, .swap])
            .map(\.asset.id.identifier)
    }

    public func getPayAssetIds(walletId: String) async throws -> [BsvHomeCore.AssetId] {
        try assetStore.getAssetsData(
            walletId: WalletId.from(id: walletId),
            filters: [.enabled, .swappable],
            limit: nil,
        ).map(\.asset.id.identifier)
    }

    public func getReceiveAssetIds(walletId: String, chains: [BsvHomeCore.Chain], assetIds: [BsvHomeCore.AssetId]) async throws -> [BsvHomeCore.AssetId] {
        try assetStore.getAssetsData(
            walletId: WalletId.from(id: walletId),
            filters: [.enabled, .swappable, .chainsOrAssets(chains, assetIds)],
            limit: nil,
        ).map(\.asset.id.identifier)
    }
}
