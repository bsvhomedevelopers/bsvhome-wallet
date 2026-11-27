// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomePreferencesService
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Localization
import Observation
import Primitives
import PrimitivesTestKit
@testable import Store
import StoreTestKit
import Testing
@testable import WalletTab
import WalletTabTestKit

@MainActor
struct WalletSceneViewModelTests {
    @Test
    func homeBalanceStaysInUsdWhenPreferencesUseAnotherCurrency() throws {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        try preferences.setCurrencyValue(.eur)
        let asset = Chain.bitcoinSv.asset
        let db = DB.mockAssets(assets: [.mock(asset: asset)])
        try BalanceStore(db: db).updateBalances([.mock(assetId: asset.id, available: 200_000_000)], for: .mock())
        try PriceStore(db: db).updatePrices([.mock(assetId: asset.id, price: 0.2)])
        let model = WalletSceneViewModel.mock(observablePreferences: .mock(preferencesService: preferences))
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        model.bsvPriceUsdQuery.bind(dbQueue: db.dbQueue)

        #expect(model.homeBalance.currency == .usd)
        #expect(model.homeBalance.secondaryDecimalValue == Decimal(string: "0.4"))
        #expect(model.homeBalance.primaryValue == 0.4)
        #expect(model.homeBalance.secondaryTitle == "BSV")
        #expect(preferences.currency == .eur)
    }

    @Test
    func homeBalanceDoesNotTurnMissingPriceIntoZero() throws {
        let asset = Chain.bitcoinSv.asset
        let db = DB.mockAssets(assets: [.mock(asset: asset)])
        try BalanceStore(db: db).updateBalances([.mock(assetId: asset.id, available: 200_000_000)], for: .mock())
        let model = WalletSceneViewModel.mock()
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        model.bsvPriceUsdQuery.bind(dbQueue: db.dbQueue)

        #expect(model.homeBalance.primaryValue == nil)
        #expect(model.homeBalance.secondaryDecimalValue == nil)
    }

    @Test
    func homeRefreshFailureKeepsCachedBalanceAndCanRetry() async throws {
        let outcomes = HomeRefreshOutcomes()
        let service = BsvHomeWalletHomeServiceMock(refreshHandler: { try await outcomes.refresh() })
        let asset = Chain.bitcoinSv.asset
        let db = DB.mockAssets(assets: [.mock(asset: asset)])
        try BalanceStore(db: db).updateBalances([.mock(assetId: asset.id, available: 200_000_000)], for: .mock())
        try PriceStore(db: db).updatePrices([.mock(assetId: asset.id, price: 0.2)])
        let model = WalletSceneViewModel.mock(service: service)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        model.bsvPriceUsdQuery.bind(dbQueue: db.dbQueue)

        await model.load()
        #expect(model.refreshState.isError)
        #expect(model.homeBalance.primaryValue == 0.4)
        await model.load()
        #expect(!model.refreshState.isLoading)
        #expect(!model.refreshState.isError)
        #expect(model.homeBalance.primaryValue == 0.4)
        #expect(await outcomes.calls == 2)
    }

    @Test
    func productScopeHidesRetainedDevelopmentAssets() throws {
        let trx = Chain.tronShasta.asset
        let usdt = Chain.tronShasta.defaultAsset(type: .trc20)
        let db = DB.mockAssets(assets: [.mock(asset: usdt), .mock(asset: trx), .mock(asset: Chain.tron.asset)])
        let model = WalletSceneViewModel.mock(developmentAssetIds: [trx.id, usdt.id], availableChains: [.tron, .tronShasta])
        model.developmentAssetsQuery.bind(dbQueue: db.dbQueue)

        #expect(!model.showsDevelopmentAssets)
        #expect(model.developmentAssets.isEmpty)
        #expect(try AssetStore(db: db).getAssets().contains { $0.id == trx.id })

        let production = WalletSceneViewModel.mock()
        production.developmentAssetsQuery.bind(dbQueue: db.dbQueue)
        #expect(!production.showsDevelopmentAssets)
        #expect(production.developmentAssets.isEmpty)

        let other = WalletSceneViewModel.mock(wallet: .mock(id: .multicoin(address: "other-wallet")), developmentAssetIds: [trx.id, usdt.id], availableChains: [.tronShasta])
        other.developmentAssetsQuery.bind(dbQueue: db.dbQueue)
        #expect(other.developmentAssets.isEmpty)
    }

    @Test
    func bothBalancesUseUsdWithoutFiatRates() async throws {
        let db = DB.mockAssets(assets: [.mock(asset: Chain.bitcoinSv.asset), .mock(asset: Chain.tron.asset)])
        try PriceStore(db: db).updatePrices([
            .mock(assetId: AssetId(chain: .bitcoinSv), price: 1100, rate: 150),
            .mock(assetId: AssetId(chain: .tron), price: 100, rate: 150),
        ])
        try BalanceStore(db: db).updateBalances([
            .mock(assetId: AssetId(chain: .bitcoinSv), available: 3),
            .mock(assetId: AssetId(chain: .tron), available: 2),
        ], for: .mock())
        let rates = FiatRateStore(db: db)
        let model = WalletSceneViewModel.mock()
        model.fiatValuesQuery.bind(dbQueue: db.dbQueue)
        model.fiatRatesQuery.bind(dbQueue: db.dbQueue)

        let total = 3300.0
        #expect(model.homeState.balance.primaryValue == total)
        #expect(model.homeState.balance.secondaryValue == total)

        try rates.add([FiatRate(symbol: .cny, rate: 7.2)])
        for _ in 0 ..< 100 where model.fiatRatesQuery.value.first?.rate != 7.2 {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(model.fiatRatesQuery.value.first?.rate == 7.2)
        #expect(model.homeState.balance.secondaryValue == total)

        #expect(model.homeState.currencyCode == "USD")
        #expect(model.homeState.balance.primaryValue == total)
        #expect(model.homeState.balance.secondaryValue == total)

        try rates.add([FiatRate(symbol: .cny, rate: 7.3)])
        for _ in 0 ..< 100 where model.fiatRatesQuery.value.first?.rate != 7.3 {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(model.fiatRatesQuery.value.first?.rate == 7.3)
        #expect(model.homeState.balance.secondaryValue == total)

        let other = WalletSceneViewModel.mock(wallet: .mock(id: .multicoin(address: "other-wallet")))
        other.fiatValuesQuery.bind(dbQueue: db.dbQueue)
        other.fiatRatesQuery.bind(dbQueue: db.dbQueue)
        #expect(other.homeState.balance.primaryValue == 0)
        #expect(other.homeState.balance.secondaryValue == 0)
    }

    @Test(arguments: [
        ("CNY", 7.2, 17.77777632, "17.77777632", "17.7777"),
        ("EUR", 0.9, 2.22222204, "2.22222204", "2.2222"),
    ])
    func selectedCurrencyConvertsTotalAndAssetHoldings(currencyCode: String, rate: Double, total: Double, holding: String, amount: String) throws {
        let currency = try #require(Currency(rawValue: currencyCode))
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        try preferences.setCurrencyValue(currency)
        let asset = Chain.bitcoinSv.asset
        let db = DB.mockAssets(assets: [.mock(asset: asset)])
        try BalanceStore(db: db).updateBalances([
            UpdateBalance(
                assetId: asset.id,
                available: UpdateBalanceValue(value: "1234567800", amount: 12.345678),
                updatedAt: .now,
                isActive: true,
            ),
        ], for: .mock())
        try PriceStore(db: db).updatePrices([.mock(assetId: asset.id, price: 0.2, rate: 150)])
        try FiatRateStore(db: db).add([FiatRate(symbol: currency, rate: rate)])
        let model = WalletSceneViewModel.mock(observablePreferences: .mock(preferencesService: preferences))
        model.fiatValuesQuery.bind(dbQueue: db.dbQueue)
        model.fiatRatesQuery.bind(dbQueue: db.dbQueue)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        model.bsvPriceUsdQuery.bind(dbQueue: db.dbQueue)

        let balance = model.balance(for: .bsv)
        let primaryValue = try #require(balance.primaryValue)
        #expect(abs(primaryValue - total) < 0.00000001)
        #expect(balance.secondaryDecimalValue == Decimal(string: holding))
        #expect(balance.secondaryAmount.hasSuffix(amount))
        #expect(balance.secondaryTitle == "BSV")
        #expect(balance.title.hasSuffix("(\(currencyCode))"))
        #expect(model.homeState.currencyCode == currencyCode)
        #expect(preferences.currency == currency)
    }

    @Test
    func selectedCurrencyWithoutRateShowsMissingAmounts() throws {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        try preferences.setCurrencyValue(.eur)
        let asset = Chain.bitcoinSv.asset
        let db = DB.mockAssets(assets: [.mock(asset: asset)])
        try BalanceStore(db: db).updateBalances([
            UpdateBalance(
                assetId: asset.id,
                available: UpdateBalanceValue(value: "200000000", amount: 2),
                updatedAt: .now,
                isActive: true,
            ),
        ], for: .mock())
        try PriceStore(db: db).updatePrices([.mock(assetId: asset.id, price: 0.2)])
        let model = WalletSceneViewModel.mock(observablePreferences: .mock(preferencesService: preferences))
        model.fiatValuesQuery.bind(dbQueue: db.dbQueue)
        model.fiatRatesQuery.bind(dbQueue: db.dbQueue)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        model.bsvPriceUsdQuery.bind(dbQueue: db.dbQueue)

        #expect(model.homeState.balance.primaryValue == nil)
        #expect(model.homeState.balance.secondaryValue == nil)
        #expect(model.homeState.balance.value == "€ --")
        #expect(model.balance(for: .bsv).secondaryDecimalValue == nil)
        #expect(model.balance(for: .bsv).secondaryAmount == "≈ € --")
        #expect(model.homeState.currencyCode == "EUR")
    }

    @Test
    func currencyChangeRevaluesBothCardsWithoutChangingSelectedCoin() throws {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let asset = Chain.bitcoinSv.asset
        let db = DB.mockAssets(assets: [.mock(asset: asset)])
        try BalanceStore(db: db).updateBalances([
            UpdateBalance(
                assetId: asset.id,
                available: UpdateBalanceValue(value: "200000000", amount: 2),
                updatedAt: .now,
                isActive: true,
            ),
        ], for: .mock())
        try PriceStore(db: db).updatePrices([.mock(assetId: asset.id, price: 0.2)])
        try FiatRateStore(db: db).add([FiatRate(symbol: .cny, rate: 7.2), FiatRate(symbol: .eur, rate: 0.9)])
        let model = WalletSceneViewModel.mock(observablePreferences: .mock(preferencesService: preferences))
        model.fiatValuesQuery.bind(dbQueue: db.dbQueue)
        model.fiatRatesQuery.bind(dbQueue: db.dbQueue)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        model.bsvPriceUsdQuery.bind(dbQueue: db.dbQueue)

        #expect(model.balance(for: .bsv).secondaryDecimalValue == Decimal(string: "0.4"))
        try preferences.setCurrencyValue(.cny)
        model.observablePreferences.reload()
        let cnyBalance = model.balance(for: .bsv)
        let cnyTotal = try #require(cnyBalance.primaryValue)
        #expect(cnyBalance.secondaryTitle == "BSV")
        #expect(cnyBalance.secondaryDecimalValue == Decimal(string: "2.88"))
        #expect(abs(cnyTotal - 2.88) < 0.00000001)

        try preferences.setCurrencyValue(.eur)
        model.observablePreferences.reload()
        let eurBalance = model.balance(for: .bsv)
        let eurTotal = try #require(eurBalance.primaryValue)
        #expect(eurBalance.secondaryTitle == "BSV")
        #expect(eurBalance.secondaryDecimalValue == Decimal(string: "0.36"))
        #expect(abs(eurTotal - 0.36) < 0.00000001)
        #expect(eurBalance.currency == .eur)
        #expect(model.balance(for: .trx).currency == .eur)
        #expect(model.balance(for: .usdt).currency == .eur)
        #expect(preferences.currency == .eur)
    }

    @Test
    func emptyWalletShowsBothDollarBalancesWithoutRates() {
        let db = DB.mockAssets(assets: [.mock()])
        let model = WalletSceneViewModel.mock()
        model.fiatValuesQuery.bind(dbQueue: db.dbQueue)
        model.fiatRatesQuery.bind(dbQueue: db.dbQueue)
        #expect(model.homeState.balance.primaryValue == 0)
        #expect(model.homeState.balance.secondaryValue == 0)
        #expect(model.homeState.balance.secondaryAmount == "≈ $0.0000")
    }

    @Test
    func renameNotifiesWalletBar() async throws {
        let wallet = Wallet.mock(id: .multicoin(address: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5"), name: "First", accounts: [.mock(chain: .bitcoinSv, address: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5")])
        let db = DB.mockWithChains([.bitcoinSv])
        let store = WalletStore.mock(db: db)
        try store.addWallet(wallet)

        let model = WalletSceneViewModel.mock(wallet: wallet)
        model.walletQuery.bind(dbQueue: db.dbQueue)

        #expect(model.walletBarModel.name == "First")

        await confirmation(expectedCount: 1...) { changed in
            withObservationTracking {
                _ = model.wallet
            } onChange: {
                changed()
            }

            try? store.renameWallet(wallet.id, name: "Renamed")
            for _ in 0 ..< 100 where model.wallet.name != "Renamed" {
                try? await Task.sleep(for: .milliseconds(10))
            }
        }

        #expect(model.walletBarModel.name == "Renamed")
    }

    @Test
    func onboardingHiddenAndSecurityBannerVisible() throws {
        let funded = try homeBannerModel(db: DB.mockAssets())
        let empty = try homeBannerModel(db: DB.mockAssets(assets: [.mock()]))

        #expect(funded.homeState.visibleBanners.map(\.event) == [.accountBlockedMultiSignature])
        #expect(empty.homeState.visibleBanners.map(\.event) == [.accountBlockedMultiSignature])
    }

    @Test
    func fixedQueryUsesOnlyBsvIdentityAndPreservesHiddenAssets() throws {
        let bsv = Asset.mock(id: AssetId(chain: .bitcoinSv), name: "Bitcoin SV", symbol: "BSV")
        let sameSymbol = Asset.mock(id: AssetId(chain: .bitcoinSv, tokenId: "different-token"), symbol: "BSV", type: .erc20)
        let usdt = Asset.mockEthereumUSDT()
        let tronUsdt = Chain.tron.defaultAsset(type: .trc20)
        let db = DB.mockAssets(assets: [.mock(asset: bsv), .mock(asset: sameSymbol), .mock(asset: usdt), .mock(asset: tronUsdt), .mock(asset: Chain.tron.asset), .mock(asset: Chain.tronShasta.asset)])
        try BalanceStore(db: db).setIsEnabled(walletId: .mock(), assetIds: [bsv.id], value: false)
        try PriceStore(db: db).updatePrices([.mock(assetId: usdt.id, price: 1), .mock(assetId: tronUsdt.id, price: 1)])
        try BalanceStore(db: db).updateBalances([.mock(assetId: usdt.id, available: 1000), .mock(assetId: tronUsdt.id, available: 75)], for: .mock())

        let model = WalletSceneViewModel.mock()
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        model.fiatValuesQuery.bind(dbQueue: db.dbQueue)
        #expect(Set(model.fixedAssetsQuery.value.map(\.asset.id)) == Set([bsv.id]))
        #expect(model.assetData(for: .trx) == nil)
        #expect(model.assetData(for: .usdt) == nil)
        #expect(model.assetData(for: .mnee) == nil)
        #expect(WowPayAsset.visibleCases == [.bsv])
        #expect(model.assetData(for: .bsv)?.metadata.isBalanceEnabled == false)
        #expect(model.homeState.balance.primaryValue == 0)
        #expect(try Set([bsv.id, sameSymbol.id, usdt.id, tronUsdt.id, Chain.tron.asset.id, Chain.tronShasta.asset.id]).isSubset(of: Set(AssetStore(db: db).getAssets().map(\.id))))

        let other = WalletSceneViewModel.mock(wallet: .mock(id: .multicoin(address: "other-wallet")))
        other.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        #expect(other.fixedAssetsQuery.value.isEmpty)
    }

    @Test
    func selectedAssetUsesTotalHoldingsAndUsdPrice() throws {
        let bsv = Asset.mock(id: AssetId(chain: .bitcoinSv), name: "Bitcoin SV", symbol: "BSV")
        let tronUsdt = Chain.tron.defaultAsset(type: .trc20)
        let ethereumUsdt = Asset.mockEthereumUSDT()
        let trx = Chain.tron.asset
        let db = DB.mockAssets(assets: [.mock(asset: bsv), .mock(asset: trx), .mock(asset: tronUsdt), .mock(asset: ethereumUsdt)])
        try BalanceStore(db: db).updateBalances([
            UpdateBalance(
                assetId: bsv.id,
                available: UpdateBalanceValue(value: "150000000", amount: 1.5),
                frozen: UpdateBalanceValue(value: "50000000", amount: 0.5),
                updatedAt: .now,
                isActive: true,
            ),
            UpdateBalance(
                assetId: trx.id,
                available: UpdateBalanceValue(value: "12345678", amount: 12.345678),
                updatedAt: .now,
                isActive: true,
            ),
            UpdateBalance(
                assetId: tronUsdt.id,
                available: UpdateBalanceValue(value: "7250000", amount: 7.25),
                updatedAt: .now,
                isActive: true,
            ),
            UpdateBalance(
                assetId: ethereumUsdt.id,
                available: UpdateBalanceValue(value: "1000000000", amount: 1000),
                updatedAt: .now,
                isActive: true,
            ),
        ], for: .mock())
        try PriceStore(db: db).updatePrices([
            .mock(assetId: bsv.id, price: 20, rate: 10),
            .mock(assetId: trx.id, price: 0.2, rate: 10),
            .mock(assetId: tronUsdt.id, price: 0.9, rate: 10),
            .mock(assetId: ethereumUsdt.id, price: 1, rate: 10),
        ])

        let model = WalletSceneViewModel.mock()
        model.fiatValuesQuery.bind(dbQueue: db.dbQueue)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        model.bsvPriceUsdQuery.bind(dbQueue: db.dbQueue)
        model.trxPriceUsdQuery.bind(dbQueue: db.dbQueue)
        model.usdtPriceUsdQuery.bind(dbQueue: db.dbQueue)

        let total = model.homeState.balance.primaryValue
        let bsvBalance = model.balance(for: .bsv)
        let trxBalance = model.balance(for: .trx)
        let usdtBalance = model.balance(for: .usdt)
        let mneeBalance = model.balance(for: .mnee)
        #expect(total == 40)
        #expect(bsvBalance.primaryValue == total)
        #expect(trxBalance.primaryValue == total)
        #expect(usdtBalance.primaryValue == total)
        #expect(mneeBalance.primaryValue == total)
        #expect(bsvBalance.secondaryTitle == "BSV")
        #expect(trxBalance.secondaryTitle == "TRX")
        #expect(usdtBalance.secondaryTitle == "USDT")
        #expect(mneeBalance.secondaryTitle == "MNEE")
        #expect(bsvBalance.secondaryAmount == "≈ $40.0000")
        #expect(trxBalance.secondaryAmount == "≈ $0.0000")
        #expect(usdtBalance.secondaryAmount == "≈ $0.0000")
        #expect(mneeBalance.secondaryAmount == "≈ $0.0000")
    }

    @Test
    func positiveHoldingsWithoutUsdPriceShowMissingAmount() throws {
        let bsv = Asset.mock(id: AssetId(chain: .bitcoinSv), name: "Bitcoin SV", symbol: "BSV")
        let tronUsdt = Chain.tron.defaultAsset(type: .trc20)
        let trx = Chain.tron.asset
        let db = DB.mockAssets(assets: [.mock(asset: bsv), .mock(asset: trx), .mock(asset: tronUsdt)])
        try BalanceStore(db: db).updateBalances([
            UpdateBalance(
                assetId: bsv.id,
                available: UpdateBalanceValue(value: "100000000", amount: 1),
                updatedAt: .now,
                isActive: true,
            ),
            UpdateBalance(
                assetId: trx.id,
                available: UpdateBalanceValue(value: "1000000", amount: 1),
                updatedAt: .now,
                isActive: true,
            ),
            UpdateBalance(assetId: tronUsdt.id, updatedAt: .now, isActive: true),
        ], for: .mock())

        let model = WalletSceneViewModel.mock()
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        model.bsvPriceUsdQuery.bind(dbQueue: db.dbQueue)
        model.trxPriceUsdQuery.bind(dbQueue: db.dbQueue)
        model.usdtPriceUsdQuery.bind(dbQueue: db.dbQueue)

        #expect(model.balance(for: .bsv).secondaryAmount == "≈ $ --")
        #expect(model.balance(for: .trx).secondaryAmount == "≈ $0.0000")
        #expect(model.balance(for: .usdt).secondaryAmount == "≈ $0.0000")

        let other = WalletSceneViewModel.mock(wallet: .mock(id: .multicoin(address: "other-wallet")))
        other.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        other.bsvPriceUsdQuery.bind(dbQueue: db.dbQueue)
        #expect(other.balance(for: .bsv).secondaryAmount == "≈ $0.0000")
    }

    @Test
    func homeReceiveRejectsMissingBsvAddress() throws {
        let wallet = Wallet.mock(accounts: [.mock(chain: .bitcoinSv, address: "  ")])
        let db = DB.mock()
        try AssetStore(db: db).add(assets: [.mock(asset: Chain.bitcoinSv.asset)])
        try WalletStore(db: db).addWallet(wallet)
        try BalanceStore(db: db).addBalance(assetIds: [Chain.bitcoinSv.asset.id], isEnabled: true, for: wallet.id)
        let model = WalletSceneViewModel.mock(wallet: wallet)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)

        #expect(model.assetData(for: .bsv) != nil)
        #expect(model.onSelectHomeReceive(asset: .bsv) == nil)
        #expect(model.isPresentingToastMessage?.title.contains("BSV") == true)
        #expect(model.isPresentingSheet == nil)
    }

    @Test(arguments: [
        (WowPayAsset.bsv, Chain.bitcoinSv.asset),
        (.trx, Chain.tron.asset),
        (.usdt, Chain.tron.defaultAsset(type: .trc20)),
    ])
    func homeActionsFollowSelectedAsset(asset: WowPayAsset, expectedAsset: Asset) throws {
        let wallet = Wallet.mock(accounts: [
            .mock(chain: .bitcoinSv, address: "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA"),
            .mock(chain: .tron, address: "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
        ])
        let model = try homeAssetModel(wallet: wallet)
        if asset != .bsv {
            #expect(model.assetData(for: asset) == nil)
            #expect(model.assetData(for: .bsv) != nil)
            #expect(model.onSelectHomeReceive(asset: asset) == nil)
            #expect(model.isPresentingToastMessage?.title == Localized.wowpayReceiveAssetUnavailable(asset.rawValue))
            #expect(model.onSelectHomeSend(asset: asset) == nil)
            #expect(model.isPresentingToastMessage?.title == Localized.wowpaySendAssetUnavailable(asset.rawValue))
            #expect(model.isPresentingSheet == nil)
            return
        }
        let expectedAccount = try #require(wallet.accounts.first { $0.chain == expectedAsset.chain })

        let receive = model.onSelectHomeReceive(asset: asset)
        #expect(receive?.asset.id == expectedAsset.id)
        #expect(receive?.account == expectedAccount)

        let send = model.onSelectHomeSend(asset: asset)
        #expect(send?.asset.id == expectedAsset.id)
        #expect(send?.account == expectedAccount)
        #expect(model.isPresentingToastMessage == nil)
    }

    @Test(arguments: [
        (WowPayAsset.bsv, Chain.bitcoinSv.asset),
        (.trx, Chain.tron.asset),
        (.usdt, Chain.tron.defaultAsset(type: .trc20)),
    ])
    func homeActionsRejectMissingSelectedAddress(asset: WowPayAsset, expectedAsset: Asset) throws {
        let wallet = Wallet.mock(accounts: [.mock(chain: expectedAsset.chain, address: "  ")])
        let model = try homeAssetModel(wallet: wallet)

        if asset == .bsv {
            #expect(model.assetData(for: asset)?.asset.id == expectedAsset.id)
        } else {
            #expect(model.assetData(for: asset) == nil)
        }
        #expect(model.onSelectHomeReceive(asset: asset) == nil)
        #expect(model.isPresentingToastMessage?.title == Localized.wowpayReceiveAssetUnavailable(asset.rawValue))
        #expect(model.onSelectHomeSend(asset: asset) == nil)
        #expect(model.isPresentingToastMessage?.title == Localized.wowpaySendAssetUnavailable(asset.rawValue))
        #expect(model.isPresentingSheet == nil)
    }

    @Test(arguments: [WowPayAsset.trx, .usdt])
    func homeActionsDoNotFallBackToBsv(asset: WowPayAsset) throws {
        let wallet = Wallet.mock(accounts: [.mock(chain: .bitcoinSv, address: "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA")])
        let model = try homeAssetModel(wallet: wallet)

        #expect(model.assetData(for: .bsv) != nil)
        #expect(model.onSelectHomeReceive(asset: asset) == nil)
        #expect(model.isPresentingToastMessage?.title == Localized.wowpayReceiveAssetUnavailable(asset.rawValue))
        #expect(model.onSelectHomeSend(asset: asset) == nil)
        #expect(model.isPresentingToastMessage?.title == Localized.wowpaySendAssetUnavailable(asset.rawValue))
        #expect(model.isPresentingSheet == nil)
    }

    @Test
    func homeReceiveReturnsCurrentWalletBsvAssetAndAddress() throws {
        let address = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA"
        let wallet = Wallet.mock(accounts: [.mock(chain: .bitcoinSv, address: address)])
        let db = DB.mock()
        try AssetStore(db: db).add(assets: [.mock(asset: Chain.bitcoinSv.asset)])
        try WalletStore(db: db).addWallet(wallet)
        try BalanceStore(db: db).addBalance(assetIds: [Chain.bitcoinSv.asset.id], isEnabled: true, for: wallet.id)
        let model = WalletSceneViewModel.mock(wallet: wallet)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)

        let selected = try #require(model.onSelectHomeReceive(asset: .bsv))
        #expect(selected.asset.id == Chain.bitcoinSv.asset.id)
        #expect(selected.account.chain == .bitcoinSv)
        #expect(selected.account.address == address)
        #expect(model.isPresentingToastMessage == nil)
    }

    @Test
    func homeSendRejectsMissingBsvAddress() throws {
        let wallet = Wallet.mock(accounts: [.mock(chain: .bitcoinSv, address: "  ")])
        let db = DB.mock()
        try AssetStore(db: db).add(assets: [.mock(asset: Chain.bitcoinSv.asset)])
        try WalletStore(db: db).addWallet(wallet)
        try BalanceStore(db: db).addBalance(assetIds: [Chain.bitcoinSv.asset.id], isEnabled: true, for: wallet.id)
        let model = WalletSceneViewModel.mock(wallet: wallet)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)

        #expect(model.assetData(for: .bsv) != nil)
        #expect(model.onSelectHomeSend(asset: .bsv) == nil)
        #expect(model.isPresentingToastMessage?.title == Localized.wowpaySendAssetUnavailable("BSV"))
        #expect(model.isPresentingSheet == nil)
    }

    @Test
    func homeSendRejectsViewOnlyWallet() throws {
        let address = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA"
        let wallet = Wallet.mock(
            id: .view(chain: .bitcoinSv, address: address),
            type: .view,
            accounts: [.mock(chain: .bitcoinSv, address: address)],
        )
        let db = DB.mock()
        try AssetStore(db: db).add(assets: [.mock(asset: Chain.bitcoinSv.asset)])
        try WalletStore(db: db).addWallet(wallet)
        try BalanceStore(db: db).addBalance(assetIds: [Chain.bitcoinSv.asset.id], isEnabled: true, for: wallet.id)
        let model = WalletSceneViewModel.mock(wallet: wallet)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)

        #expect(model.onSelectHomeSend(asset: .bsv) == nil)
        #expect(model.isPresentingToastMessage?.title == Localized.wowpaySendAssetUnavailable("BSV"))
        #expect(model.isPresentingSheet == nil)
    }

    @Test
    func homeSendReturnsCurrentWalletBsvAssetAndAddress() throws {
        let address = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA"
        let wallet = Wallet.mock(accounts: [.mock(chain: .bitcoinSv, address: address)])
        let db = DB.mock()
        try AssetStore(db: db).add(assets: [.mock(asset: Chain.bitcoinSv.asset)])
        try WalletStore(db: db).addWallet(wallet)
        try BalanceStore(db: db).addBalance(assetIds: [Chain.bitcoinSv.asset.id], isEnabled: true, for: wallet.id)
        let model = WalletSceneViewModel.mock(wallet: wallet)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)

        let selected = try #require(model.onSelectHomeSend(asset: .bsv))
        #expect(selected.asset.id == Chain.bitcoinSv.asset.id)
        #expect(selected.account.chain == .bitcoinSv)
        #expect(selected.account.address == address)
        #expect(model.isPresentingToastMessage == nil)
        #expect(model.isPresentingSheet == nil)
    }

    private func homeAssetModel(wallet: Wallet) throws -> WalletSceneViewModel {
        let assets = [Chain.bitcoinSv.asset, Chain.tron.asset, Chain.tron.defaultAsset(type: .trc20)]
        let db = DB.mock()
        try AssetStore(db: db).add(assets: assets.map { .mock(asset: $0) })
        try WalletStore(db: db).addWallet(wallet)
        try BalanceStore(db: db).addBalance(assetIds: assets.map(\.id), isEnabled: true, for: wallet.id)
        let model = WalletSceneViewModel.mock(wallet: wallet)
        model.fixedAssetsQuery.bind(dbQueue: db.dbQueue)
        return model
    }

    private func homeBannerModel(db: DB) throws -> WalletSceneViewModel {
        let wallet = Wallet.mock()
        try BannerStore(db: db).addBanners([
            NewBanner(id: "onboarding", walletId: wallet.id.id, event: .onboarding, state: .active),
            NewBanner(id: "blocked", walletId: wallet.id.id, event: .accountBlockedMultiSignature, state: .alwaysActive),
        ])
        let model = WalletSceneViewModel.mock(wallet: wallet)
        model.assetsQuery.bind(dbQueue: db.dbQueue)
        model.bannersQuery.bind(dbQueue: db.dbQueue)
        return model
    }
}

private actor HomeRefreshOutcomes {
    enum Failure: Error { case unavailable }
    private(set) var calls = 0

    func refresh() throws {
        calls += 1
        if calls == 1 {
            throw Failure.unavailable
        }
    }
}
