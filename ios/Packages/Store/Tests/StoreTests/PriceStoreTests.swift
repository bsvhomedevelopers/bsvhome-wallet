// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit
import Testing

struct PriceStoreTests {
    @Test
    func convertPricesDropsFiatMarketCacheAndKeepsAssetData() throws {
        let db = DB.mockWithChains([.ethereum])
        let store = PriceStore(db: db)
        let assetId = Chain.ethereum.assetId
        let date = Date(timeIntervalSince1970: 1_700_000_000)
        let market = AssetMarket.mock(
            allTimeHighValue: ChartValuePercentage(date: date, value: 100, percentage: -10),
            allTimeLowValue: ChartValuePercentage(date: date, value: 1, percentage: 100),
        )
        try store.updatePrices([.mock(assetId: assetId, price: 2500, priceChangePercentage24h: 1.5, rate: 7.2, updatedAt: date)])
        try store.updateMarket(assetId: assetId.identifier, market: market)

        try store.convertPrices(rate: 0.9)

        let data = try #require(try db.dbQueue.read { try PriceRequest(assetId: assetId, environment: .production).fetch($0) })
        #expect(data.price?.price == 2250)
        #expect(data.price?.priceChangePercentage24h == 1.5)
        #expect(data.price?.updatedAt == date)
        #expect(data.market?.marketCap == nil)
        #expect(data.market?.marketCapFdv == nil)
        #expect(data.market?.totalVolume == nil)
        #expect(data.market?.allTimeHighValue == nil)
        #expect(data.market?.allTimeLowValue == nil)
        #expect(data.market?.marketCapRank == market.marketCapRank)
        #expect(data.market?.circulatingSupply == market.circulatingSupply)
        #expect(data.market?.totalSupply == market.totalSupply)
        #expect(data.market?.maxSupply == market.maxSupply)

        try store.updateMarket(assetId: assetId.identifier, market: market)
        let refreshed = try db.dbQueue.read { try PriceRequest(assetId: assetId, environment: .production).fetch($0) }
        #expect(refreshed?.market == market)
    }

    @Test
    func convertPricesRecomputesFiatPriceFromUsd() throws {
        let db = DB.mockWithChains([.ethereum])
        let priceStore = PriceStore(db: db)
        let assetId = Chain.ethereum.assetId
        let priceUsd = 2500.0

        try priceStore.updatePrices([.mock(assetId: assetId, price: priceUsd, rate: 90)])
        #expect(try priceStore.getPrices(for: [assetId.identifier]).first?.price == priceUsd * 90)
        #expect(try priceStore.getUsdPrice(for: assetId) == priceUsd)

        try priceStore.convertPrices(rate: 2)
        #expect(try priceStore.getPrices(for: [assetId.identifier]).first?.price == priceUsd * 2)
        #expect(try priceStore.getUsdPrice(for: assetId) == priceUsd)
    }

    @Test
    func getPricesReportsAStoredZeroPrice() throws {
        let db = DB.mockWithChains([.ethereum])
        let priceStore = PriceStore(db: db)
        let assetId = Chain.ethereum.assetId

        try priceStore.updatePrices([.mock(assetId: assetId, price: 0)])

        #expect(try priceStore.getPrices(for: [assetId.identifier]).first?.price == 0)
    }

    @Test
    func insertKeepsTheUpdateTimestamp() throws {
        let db = DB.mockWithChains([.ethereum])
        let priceStore = PriceStore(db: db)
        let assetId = Chain.ethereum.assetId
        let updatedAt = Date(timeIntervalSince1970: 1_700_000_000)

        try priceStore.updatePrices([.mock(assetId: assetId, updatedAt: updatedAt)])

        #expect(try priceStore.getPrices(for: [assetId.identifier]).first?.updatedAt == updatedAt)
    }
}
