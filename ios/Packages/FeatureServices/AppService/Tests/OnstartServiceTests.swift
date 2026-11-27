@testable import AppService
import Foundation
import class BsvHomeCore.BsvHomePreferencesService
import BsvHomeCoreServices
import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit
import Testing

struct OnstartServiceTests {
    @Test(arguments: ["CNY", "EUR", "USD", nil] as [String?])
    func startupPreservesCurrencyAndCachedPrices(currency: String?) throws {
        let suiteName = "OnstartServiceTests.\(UUID().uuidString)"
        let groupSuiteName = "\(suiteName).group"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        let groupDefaults = try #require(UserDefaults(suiteName: groupSuiteName))
        defer {
            defaults.removePersistentDomain(forName: suiteName)
            groupDefaults.removePersistentDomain(forName: groupSuiteName)
        }
        defaults.set(currency, forKey: "currency")
        groupDefaults.set("CNY", forKey: "currency")
        let preferences = BsvHomePreferencesService(store: BsvHomeCorePreferencesStore.application(userDefaults: defaults, appGroupDefaults: groupDefaults))
        let db = DB.mockWithChains([.ethereum])
        let prices = PriceStore(db: db)
        let assetId = Chain.ethereum.assetId
        let rate = currency == "CNY" ? 7.2 : currency == "EUR" ? 0.9 : 1.0
        try prices.updatePrices([.mock(assetId: assetId, price: 2500, rate: rate)])
        try prices.updateMarket(assetId: assetId.identifier, market: .mock())

        try OnstartService.configureCurrency(preferencesService: preferences)

        #expect(preferences.getCurrency() == (currency ?? "USD"))
        #expect(defaults.string(forKey: "currency") == (currency ?? "USD"))
        #expect(groupDefaults.string(forKey: "currency") == (currency ?? "USD"))
        #expect(try prices.getPrices(for: [assetId.identifier]).first?.price == 2500 * rate)
        let market = try db.dbQueue.read { try PriceRequest(assetId: assetId, environment: .production).fetch($0)?.market }
        #expect(market?.marketCap == 1_000_000)
        try OnstartService.configureCurrency(preferencesService: preferences)
        #expect(try prices.getPrices(for: [assetId.identifier]).first?.price == 2500 * rate)
    }
}
