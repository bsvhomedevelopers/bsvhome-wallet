import Foundation
import Primitives
import Testing
@testable import WalletTab

struct WalletBalanceViewModelTests {
    @Test(arguments: [
        ("CNY", "¥2,580.0000", "≈ CN¥1,234.5678"),
        ("EUR", "€2,580.0000", "≈ €1,234.5678"),
    ])
    func selectedCurrencyFormatsBothAmounts(currencyCode: String, value: String, secondaryAmount: String) throws {
        let currency = try #require(Currency(rawValue: currencyCode))
        let model = WalletBalanceViewModel(
            primaryValue: 2580,
            secondaryValue: nil,
            currency: currency,
            locale: Locale(identifier: "en_US"),
            secondaryTitle: "TRX",
            secondaryDecimalValue: Decimal(string: "1234.56789"),
        )

        #expect(model.value == value)
        #expect(model.secondaryAmount == secondaryAmount)
        #expect(model.title.hasSuffix("(\(currencyCode))"))
        #expect(model.secondaryTitle == "TRX")
    }

    @Test
    func missingSelectedCurrencyAmountKeepsItsSymbol() {
        let model = WalletBalanceViewModel(primaryValue: nil, secondaryValue: nil, currency: .eur, locale: Locale(identifier: "en_US"))

        #expect(model.value == "€ --")
        #expect(model.secondaryAmount == "≈ € --")
        #expect(model.secondaryTitle == "EUR")
    }

    @Test(arguments: ["zh_CN", "zh_TW", "en_US"])
    func usdUsesDollarSymbol(locale: String) {
        let model = WalletBalanceViewModel(primaryValue: 0, secondaryValue: 0, locale: Locale(identifier: locale))
        #expect(model.value == "$0.0000")
        #expect(model.secondaryAmount == "≈ $0.0000")
        let missing = WalletBalanceViewModel(primaryValue: nil, secondaryValue: nil, locale: Locale(identifier: locale))
        #expect(missing.value == "$ --")
        #expect(missing.secondaryAmount == "≈ $ --")
    }

    @Test
    func formatsBothDollarAmounts() {
        let model = WalletBalanceViewModel(primaryValue: 2580, secondaryValue: 2580, locale: Locale(identifier: "en_US"))
        #expect(model.value == "$2,580.0000")
        #expect(model.title.hasSuffix("(USD)"))
        #expect(model.secondaryTitle == "USD")
        #expect(model.secondaryAmount == "≈ $2,580.0000")
        let missing = WalletBalanceViewModel(primaryValue: 2580, secondaryValue: nil)
        #expect(missing.secondaryAmount.hasSuffix("--"))
    }

    @Test
    func bothCardAmountsTruncateToFourDecimals() {
        let model = WalletBalanceViewModel(
            primaryValue: 9.996041249809775,
            secondaryValue: 1234.56789,
            locale: Locale(identifier: "en_US"),
        )

        #expect(model.value == "$9.9960")
        #expect(model.secondaryAmount == "≈ $1,234.5678")
    }

    @Test
    func smallAndNegativeCardAmountsTruncateTowardZero() {
        let model = WalletBalanceViewModel(
            primaryValue: -1.234567,
            secondaryValue: 0.00009,
            locale: Locale(identifier: "en_US"),
        )

        #expect(model.value == "-$1.2345")
        #expect(model.secondaryAmount == "≈ $0.0000")
    }
}
