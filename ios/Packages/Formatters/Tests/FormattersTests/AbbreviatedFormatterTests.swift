// Copyright (c). Gem Wallet. All rights reserved.

@testable import Formatters
import Foundation
import Testing

struct AbbreviatedFormatterTests {
    let formatter = AbbreviatedFormatter(locale: .US, threshold: defaultAbbreviationThreshold)

    @Test
    func abbreviatedFormatter() {
        #expect(formatter.string(from: 99999.0) == nil)
        #expect(formatter.string(from: 100_000.0) == "100.0000K")
        #expect(formatter.string(from: 123_456.0) == "123.4560K")
        #expect(formatter.string(from: 1_500_000.0) == "1.5000M")
        #expect(formatter.string(from: 2_300_000_000.0) == "2.3000B")
        #expect(formatter.string(from: 1_200_000_000_000.0) == "1.2000T")
        #expect(formatter.string(from: 100_000.0, currency: "USD") == "$100.0000K")
        #expect(formatter.string(from: 2_000_000.0, currency: "EUR") == "€2.0000M")
        #expect(formatter.string(from: 2_500_000.0, currency: "USD") == "$2.5000M")
    }

    @Test
    func keepsDigitsWithoutRounding() {
        #expect(formatter.string(from: 267_123.0) == "267.1230K")
        #expect(formatter.string(from: 20_070_000.0) == "20.0700M")
        #expect(formatter.string(from: 19_876_725.0) == "19.876725M")
        #expect(formatter.string(from: 267_123.0, currency: "USD") == "$267.1230K")
        #expect(formatter.string(from: 123_456.123456) == "123.456123456K")
    }

    @Test
    func customThreshold() {
        let formatter = AbbreviatedFormatter(locale: .US, threshold: 1000.0)

        #expect(formatter.string(from: 1500.0) == "1.5000K")
        #expect(formatter.string(from: 500.0) == nil)
    }
}
