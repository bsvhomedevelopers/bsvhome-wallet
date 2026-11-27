// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Formatters
import Foundation
import Testing

struct NumericFormatterTests {
    let formatter = NumericFormatter(locale: .US)
    let ua = NumericFormatter(locale: .RU_UA)

    @Test
    func decimal() {
        #expect(formatter.string(0) == "0.0000")
        #expect(formatter.string(11.12) == "11.1200")
        #expect(formatter.string(11) == "11.0000")
        #expect(formatter.string(12_000_123) == "12,000,123.0000")

        #expect(formatter.string(0.12) == "0.1200")
        #expect(formatter.string(0.00012) == "0.00012")
        #expect(formatter.string(0.0000000002) == "0.0000000002")
        #expect(formatter.string(0.0000000000001) == "0.0000000000001")
        #expect(formatter.string(-0.0) == "-0.0000")
    }

    @Test
    func exactDecimal() throws {
        let amount = try #require(Decimal(string: "1234.567890123456789012345678", locale: .US))
        let uaAmount = try #require(Decimal(string: "29.73", locale: .US))

        #expect(formatter.string(decimal: 0) == "0.0000")
        #expect(formatter.string(decimal: amount) == "1,234.567890123456789012345678")
        #expect(formatter.string(decimal: amount, symbol: "BTC") == "1,234.567890123456789012345678 BTC")
        #expect(ua.string(decimal: uaAmount) == "29,7300")
    }

    @Test
    func exactAtomicAmount() {
        let amount = BigInt("123456789012345678901234567890123456789")

        #expect(formatter.string(amount, decimals: 38, symbol: "BTC") == "1.23456789012345678901234567890123456789 BTC")
    }

    @Test
    func withSymbol() {
        #expect(formatter.string(1234.56, symbol: "BTC") == "1,234.5600 BTC")
        #expect(formatter.string(0.0001234, symbol: "BTC") == "0.0001234 BTC")
        #expect(formatter.string(0, symbol: "BTC") == "0.0000 BTC")
    }

    @Test
    func uaLocale() {
        #expect(ua.string(29.73) == "29,7300")
    }
}
