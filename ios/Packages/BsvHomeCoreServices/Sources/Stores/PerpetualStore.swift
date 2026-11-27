// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.PerpetualProvider
import struct BsvHomeCore.PerpetualData
import struct BsvHomeCore.PerpetualMarketData
import struct BsvHomeCore.PerpetualPosition
import protocol BsvHomeCore.BsvHomePerpetualStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCorePerpetualStore: BsvHomePerpetualStore, @unchecked Sendable {
    private let store: PerpetualStore
    private let balanceStore: BalanceStore

    public init(store: PerpetualStore, balanceStore: BalanceStore) {
        self.store = store
        self.balanceStore = balanceStore
    }

    public func savePerpetuals(data: [BsvHomeCore.PerpetualData]) async throws {
        try store.upsertPerpetuals(data.map { $0.map().perpetual })
    }

    public func setPinned(perpetualIds: [String], pinned: Bool) async throws {
        try store.setPinned(for: perpetualIds, value: pinned)
    }

    public func deletePerpetuals() async throws {
        try store.clear()
        try balanceStore.deleteBalance(assetId: Chain.hyperCore.defaultAsset(type: .perpetual).id)
    }

    public func getPositions(walletId: String, provider: BsvHomeCore.PerpetualProvider) async throws -> [BsvHomeCore.PerpetualPosition] {
        try store.getPositions(walletId: WalletId.from(id: walletId), provider: provider.map()).map { $0.map() }
    }

    public func updateMarket(market: BsvHomeCore.PerpetualMarketData) async throws {
        try store.updateMarket(
            coin: market.coin,
            price: market.price,
            pricePercentChange24h: market.pricePercentChange24h,
            openInterest: market.openInterest,
            volume24h: market.volume24h,
            funding: market.funding,
        )
    }

    public func updatePrices(prices: [String: Double]) async throws {
        try store.updatePrices(prices)
    }

    public func getPositionIds(walletId: String, provider: BsvHomeCore.PerpetualProvider) async throws -> [String] {
        try store.getPositions(walletId: WalletId.from(id: walletId), provider: provider.map()).map(\.id)
    }

    public func updatePositions(walletId: String, positions: [BsvHomeCore.PerpetualPosition], deleteIds: [String]) async throws {
        try store.diffPositions(
            deleteIds: deleteIds,
            positions: positions.map { $0.map() },
            walletId: WalletId.from(id: walletId),
        )
    }
}
