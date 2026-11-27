import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit
import Testing
@testable import WalletTab

struct WowPayWalletAssetViewModelTests {
    @Test(arguments: [
        ("CNY", 720.0, "1,080.0000", "¥"),
        ("EUR", 90.0, "135.0000", "€"),
    ])
    func selectedCurrencyUsesItsConvertedPrice(currencyCode: String, price: Double, amount: String, symbol: String) {
        let model = WowPayWalletAssetViewModel(
            asset: .bsv,
            assetData: .mock(asset: bsv, balance: .mock(available: 150_000_000), price: .mock(price: price)),
            currencyCode: currencyCode,
            showBalancePrivacy: .constant(false),
        )

        #expect(model.balanceText == "1.5000")
        #expect(model.fiatBalanceText.hasSuffix(amount))
        #expect(model.fiatBalanceText.contains(symbol))
        #expect(!model.fiatBalanceText.contains("$"))
    }

    @Test
    func bsvBalanceKeepsPrecisionWithoutInventingPrice() {
        let model = WowPayWalletAssetViewModel(
            asset: .bsv,
            assetData: .mock(asset: bsv, balance: .mock(available: 123_456_789)),
            currencyCode: Currency.usd.rawValue,
            showBalancePrivacy: .constant(false),
        )
        #expect(model.balanceText == "1.23456789")
        #expect(model.fiatBalanceText == "—")
        #expect(model.assetData?.asset.id == bsv.id)
    }

    @Test
    func realZeroBalanceWithPriceShowsZeroDollars() {
        let model = WowPayWalletAssetViewModel(
            asset: .bsv,
            assetData: .mock(asset: bsv, balance: .zero, price: .mock(price: 100)),
            currencyCode: Currency.usd.rawValue,
            showBalancePrivacy: .constant(false),
        )
        #expect(model.balanceText == "0.0000")
        #expect(model.fiatBalanceText == "$0.0000")
    }

    @Test
    func pricedBalanceUsesDollars() {
        let model = WowPayWalletAssetViewModel(
            asset: .bsv,
            assetData: .mock(asset: bsv, balance: .mock(available: 150_000_000), price: .mock(price: 100)),
            currencyCode: Currency.usd.rawValue,
            showBalancePrivacy: .constant(false),
        )
        #expect(model.balanceText == "1.5000")
        #expect(model.fiatBalanceText == "$150.0000")
    }

    @Test
    func tronUsdtBalanceShowsFourDecimalsAndRealDollarPrice() {
        let usdt = Chain.tron.defaultAsset(type: .trc20)
        let model = WowPayWalletAssetViewModel(
            asset: .usdt,
            assetData: .mock(asset: usdt, balance: .mock(available: 123_456_789), price: .mock(price: 1)),
            currencyCode: Currency.usd.rawValue,
            showBalancePrivacy: .constant(false),
        )
        #expect(model.balanceText == "123.4567")
        #expect(model.fiatBalanceText == "$123.456789")
        #expect(model.assetData?.asset.type == .trc20)
        #expect(model.assetData?.asset.chain == .tron)
    }

    @Test
    func trxBalanceKeepsNativePrecisionAndRealDollarValue() {
        let model = WowPayWalletAssetViewModel(
            asset: .trx,
            assetData: .mock(asset: Chain.tron.asset, balance: .mock(available: 12_345_678), price: .mock(price: 0.2)),
            currencyCode: Currency.usd.rawValue,
            showBalancePrivacy: .constant(false),
        )
        #expect(model.balanceText == "12.345678")
        #expect(model.fiatBalanceText == "$2.4691356")
    }

    @Test
    func trxZeroBalanceDoesNotRequirePrice() {
        let model = WowPayWalletAssetViewModel(
            asset: .trx,
            assetData: .mock(asset: Chain.tron.asset, balance: .zero),
            currencyCode: Currency.usd.rawValue,
            showBalancePrivacy: .constant(false),
        )
        #expect(model.balanceText == "0.0000")
        #expect(model.fiatBalanceText == "—")
    }

    @Test
    func mneeRemainsUnavailable() {
        let model = WowPayWalletAssetViewModel(asset: .mnee, assetData: nil, currencyCode: Currency.usd.rawValue, showBalancePrivacy: .constant(false))
        #expect(model.assetData == nil)
        #expect(model.balanceText == "—")
        #expect(model.fiatBalanceText == "—")
    }

    @Test
    func missingBsvDoesNotInventZeroBalance() {
        let model = WowPayWalletAssetViewModel(asset: .bsv, assetData: nil, currencyCode: Currency.usd.rawValue, showBalancePrivacy: .constant(false))
        #expect(model.balanceText == "—")
        #expect(model.fiatBalanceText == "—")
    }

    private var bsv: Asset {
        .mock(id: AssetId(chain: .bitcoinSv), name: "Bitcoin SV", symbol: "BSV")
    }
}
