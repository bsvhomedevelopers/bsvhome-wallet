// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import typealias BsvHomeCore.AssetId
import struct BsvHomeCore.AssetMarket
import typealias BsvHomeCore.Currency
import struct BsvHomeCore.FiatRate
import struct BsvHomeCore.AssetPrice
import protocol BsvHomeCore.BsvHomePriceStore
import struct BsvHomeCore.BsvHomePriceUpdate
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCorePriceStore: BsvHomePriceStore, @unchecked Sendable {
    private let priceStore: PriceStore
    private let fiatRateStore: FiatRateStore

    public init(priceStore: PriceStore, fiatRateStore: FiatRateStore) {
        self.priceStore = priceStore
        self.fiatRateStore = fiatRateStore
    }

    public func getPrices(assetIds: [BsvHomeCore.AssetId]) throws -> [BsvHomeCore.AssetPrice] {
        try priceStore.getPrices(for: assetIds).map { $0.map() }
    }

    public func getRate(currency: BsvHomeCore.Currency) async throws -> BsvHomeCore.FiatRate? {
        let currency = try Primitives.Currency(id: currency)
        return try priceStore.getRate(currency: currency.rawValue).map { Primitives.FiatRate(symbol: currency, rate: $0.rate).map() }
    }

    public func getRates() async throws -> [BsvHomeCore.FiatRate] {
        try priceStore.getRates().map { Primitives.FiatRate(symbol: $0.symbol, rate: $0.rate).map() }
    }

    public func saveRates(rates: [BsvHomeCore.FiatRate]) async throws {
        try fiatRateStore.add(rates.map { $0.map() })
    }

    public func savePrices(currency _: BsvHomeCore.Currency, prices: [BsvHomePriceUpdate]) async throws {
        try priceStore.updatePrices(prices.map { update in
            try PriceUpdate(
                assetId: Primitives.AssetId(id: update.assetId),
                price: update.price,
                priceUsd: update.priceUsd,
                priceChangePercentage24h: update.priceChangePercentage24h,
                updatedAt: update.updatedAt,
            )
        })
    }

    public func convertPrices(currency _: BsvHomeCore.Currency, rate: Double) async throws {
        try priceStore.convertPrices(rate: rate)
    }

    public func saveMarket(assetId: BsvHomeCore.AssetId, market: BsvHomeCore.AssetMarket) async throws {
        try priceStore.updateMarket(assetId: assetId, market: market.map())
    }
}
