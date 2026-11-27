// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import Foundation
@testable import Primitives
import Testing

final class CurrencyFormatterTests {
    @Test(arguments: ["zh_CN", "zh_TW", "en_US", "en_GB", "fr_FR"])
    func usdAlwaysUsesDollarSymbol(locale: String) {
        for type in [CurrencyFormatterType.currency, .fiat, .fiatNarrowSymbol, .abbreviated] {
            let formatter = CurrencyFormatter(type: type, locale: Locale(identifier: locale), currencyCode: "USD")
            #expect(formatter.symbol == "$")
            for value in [0.0, 0.000123, 1234.56, -1234.56, 1_234_567.0] {
                let text = formatter.string(value)
                #expect(text.contains("$"))
                #expect(!text.contains("US"))
            }
        }
    }

    @Test
    func narrowFiatSymbolKeepsPrecisionAndLocale() {
        let locale = Locale(identifier: "zh_CN")
        let narrow = CurrencyFormatter(type: .fiatNarrowSymbol, locale: locale, currencyCode: "USD")
        #expect(narrow.string(0) == "$0.0000")
        #expect(narrow.string(1234.56) == "$1,234.5600")
        #expect(narrow.string(-1234.56) == "-$1,234.5600")
        #expect(narrow.string(0.1) == "$0.1000")
        #expect(narrow.string(0.123456) == "$0.123456")
        #expect(narrow.symbol == "$")
        #expect(CurrencyFormatter(type: .fiat, locale: locale, currencyCode: "USD").string(0) == "$0.0000")
    }

    @Test
    func balanceStringTruncatesTowardZeroWithoutChangingOtherAmounts() {
        let formatter = CurrencyFormatter(type: .fiatNarrowSymbol, locale: .US, currencyCode: "USD")

        #expect(formatter.balanceString(9.996041249809775) == "$9.9960")
        #expect(formatter.balanceString(-9.996041249809775) == "-$9.9960")
        #expect(formatter.balanceString(0.00009) == "$0.0000")
        #expect(formatter.balanceString(1234.56789) == "$1,234.5678")
        #expect(formatter.balanceString(0) == "$0.0000")
        #expect(formatter.string(9.996041249809775) == "$9.996041249809775")
        #expect(formatter.balanceString(-0.0) == formatter.string(-0.0))
        #expect(formatter.balanceString(.infinity) == formatter.string(.infinity))
    }

    @Test
    func decimalBalanceStringTruncatesExactValues() throws {
        let formatter = CurrencyFormatter(type: .fiatNarrowSymbol, locale: .US, currencyCode: "USD")
        let precise = try #require(Decimal(string: "1234.567890123456789012345678", locale: .US))
        let tiny = try #require(Decimal(string: "0.00009", locale: .US))
        let negative = try #require(Decimal(string: "-9.996041249809775", locale: .US))

        #expect(formatter.balanceString(decimal: precise) == "$1,234.5678")
        #expect(formatter.balanceString(decimal: tiny) == "$0.0000")
        #expect(formatter.balanceString(decimal: negative) == "-$9.9960")
        #expect(abbreviatedFormatterUS.balanceString(decimal: precise * 1000) == "$1.2345M")
    }

    @Test
    func balanceStringPreservesLocaleAndAbbreviation() {
        let british = CurrencyFormatter(locale: .UK, currencyCode: "GBP")
        let french = CurrencyFormatter(locale: Locale(identifier: "fr_FR"), currencyCode: "EUR")

        #expect(british.balanceString(1234.56789) == "£1,234.5678")
        #expect(french.balanceString(1234.56789).contains("1 234,5678"))
        #expect(french.balanceString(1234.56789).contains("€"))
        #expect(abbreviatedFormatterUS.balanceString(1_234_567.890123) == "$1.2345M")
        #expect(abbreviatedFormatterUS.string(1_234_567.890123) == "$1.234567890123M")
    }

    let currencyFormatterUS = CurrencyFormatter(locale: .US, currencyCode: Currency.usd.rawValue)
    let currencyFormatterUK = CurrencyFormatter(locale: .UK, currencyCode: Currency.gbp.rawValue)

    let abbreviatedFormatterUS = CurrencyFormatter(type: .abbreviated, locale: .US, currencyCode: Currency.usd.rawValue)
    let abbreviatedFormatterUK = CurrencyFormatter(type: .abbreviated, locale: .UK, currencyCode: Currency.gbp.rawValue)

    @Test
    func currency() {
        #expect(currencyFormatterUS.string(0) == "$0.0000")
        #expect(currencyFormatterUS.string(11.12) == "$11.1200")
        #expect(currencyFormatterUS.string(11) == "$11.0000")
        #expect(currencyFormatterUS.string(-1.2) == "-$1.2000")
        #expect(currencyFormatterUS.string(12_000_123) == "$12,000,123.0000")
        #expect(currencyFormatterUS.string(0.0000000002) == "$0.0000000002")
        #expect(currencyFormatterUS.string(0.0000000000001) == "$0.0000000000001")
        #expect(currencyFormatterUS.string(-0.0) == "-$0.0000")
        #expect(currencyFormatterUS.signedNumber(0) == "+0.0000")
        #expect(currencyFormatterUS.signedNumber(0.123456) == "+0.123456")
        #expect(currencyFormatterUS.signedNumber(-0.0) == "-0.0000")
    }

    @Test
    func decimalRetainsExactDigits() throws {
        let amount = try #require(Decimal(string: "1234.567890123456789012345678", locale: .US))
        let compactAmount = try #require(Decimal(string: "1234567.890123456789", locale: .US))

        #expect(currencyFormatterUS.string(decimal: 0) == "$0.0000")
        #expect(currencyFormatterUS.string(decimal: amount) == "$1,234.567890123456789012345678")
        #expect(currencyFormatterUS.signedNumber(decimal: amount) == "+1,234.567890123456789012345678")
        #expect(abbreviatedFormatterUS.string(decimal: compactAmount) == "$1.234567890123456789M")
    }

    @Test
    func smallValue() {
        #expect(currencyFormatterUS.string(0.10) == "$0.1000")
        #expect(currencyFormatterUS.string(0.11) == "$0.1100")
        #expect(currencyFormatterUS.string(0.2) == "$0.2000")
        #expect(currencyFormatterUS.string(0.99) == "$0.9900")
        #expect(currencyFormatterUS.string(1.89999) == "$1.89999")
        #expect(currencyFormatterUS.string(0.70) == "$0.7000")
        #expect(currencyFormatterUS.string(0.0345) == "$0.0345")
        #expect(currencyFormatterUS.string(0.01) == "$0.0100")
        #expect(currencyFormatterUS.string(0.13) == "$0.1300")
        #expect(currencyFormatterUS.string(0.0123) == "$0.0123")
        #expect(currencyFormatterUS.string(0.002) == "$0.0020")
        #expect(currencyFormatterUS.string(0.001) == "$0.0010")
        #expect(currencyFormatterUS.string(0.000123456) == "$0.000123456")
        #expect(currencyFormatterUS.string(0.00000123) == "$0.00000123")
    }

    @Test
    func currencyGBPLocale() {
        #expect(currencyFormatterUK.string(0.0002) == "£0.0002")
        #expect(currencyFormatterUK.string(11.12) == "£11.1200")
        #expect(currencyFormatterUK.string(11) == "£11.0000")
        #expect(currencyFormatterUK.string(12_000_123) == "£12,000,123.0000")
    }

    @Test
    func testAbbreviated() {
        #expect(abbreviatedFormatterUS.string(0) == "$0.0000")
        #expect(abbreviatedFormatterUS.string(12) == "$12.0000")
        #expect(abbreviatedFormatterUS.string(1234) == "$1,234.0000")
        #expect(abbreviatedFormatterUS.string(10000) == "$10,000.0000")
        #expect(abbreviatedFormatterUS.string(-1234) == "-$1,234.0000")
        #expect(abbreviatedFormatterUS.string(-10000) == "-$10,000.0000")
        #expect(abbreviatedFormatterUS.string(-5_600_000) == "-$5.6000M")

        #expect(abbreviatedFormatterUK.string(123_456) == "£123.4560k")
        #expect(abbreviatedFormatterUK.string(5_000_000) == "£5.0000m")
        #expect(abbreviatedFormatterUK.string(7_890_000_000) == "£7.8900bn")
        #expect(abbreviatedFormatterUK.string(1_200_000_000_000) == "£1.2000tn")
        #expect(abbreviatedFormatterUK.string(-9_999_999_999) == "-£9.999999999bn")
        #expect(abbreviatedFormatterUS.string(1_234_567.890123) == "$1.234567890123M")
    }
}
