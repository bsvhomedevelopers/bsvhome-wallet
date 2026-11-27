// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import Primitives
@testable import PrimitivesComponents
import Testing

struct PriceViewModelTests {
    let currencyCode = Currency.usd.id

    @Test
    func priceChangeAmountUsesThePreviousPrice() {
        #expect(PriceViewModel(price: .mock(price: 120, priceChangePercentage24h: 20), currencyCode: currencyCode).priceChangeAmountText == "+20.0000")
        #expect(PriceViewModel(price: .mock(price: 80, priceChangePercentage24h: -20), currencyCode: currencyCode).priceChangeAmountText == "-20.0000")
        #expect(PriceViewModel(price: .mock(price: 0.0099, priceChangePercentage24h: 10), currencyCode: currencyCode).priceChangeAmountText == "+0.0009")
        #expect(PriceViewModel(price: .mock(price: 0.0099, priceChangePercentage24h: -10), currencyCode: currencyCode).priceChangeAmountText == "-0.0011")
        #expect(PriceViewModel(price: nil, currencyCode: currencyCode).priceChangeAmountText == "—")
        #expect(PriceViewModel(price: .mock(price: 0), currencyCode: currencyCode).priceChangeAmountText == "—")
        #expect(PriceViewModel(price: .mock(price: 1, priceChangePercentage24h: -100), currencyCode: currencyCode).priceChangeAmountText == "—")
    }

    @Test
    func testIsPriceAvailable() {
        #expect(PriceViewModel(price: nil, currencyCode: currencyCode).isPriceAvailable == false)
        #expect(PriceViewModel(price: .mock(price: 0, priceChangePercentage24h: 0), currencyCode: currencyCode).isPriceAvailable == false)
        #expect(PriceViewModel(price: .mock(price: 10), currencyCode: currencyCode).isPriceAvailable)
    }

    @Test
    func testPriceAmountText() {
        #expect(PriceViewModel(price: .mock(price: 1.2), currencyCode: currencyCode).priceAmountText == "$1.2000")
        #expect(PriceViewModel(price: .mock(price: 10), currencyCode: currencyCode).priceAmountText == "$10.0000")
        #expect(PriceViewModel(price: .mock(price: 0.000000123), currencyCode: currencyCode).priceAmountText == "$0.000000123")
        #expect(PriceViewModel(price: .mock(price: 0.00000000123), currencyCode: currencyCode).priceAmountText == "$0.00000000123")
        #expect(PriceViewModel(price: .mock(price: 0.000000123456), currencyCode: currencyCode).priceAmountText == "$0.000000123456")
        #expect(PriceViewModel(price: .mock(price: -10), currencyCode: currencyCode).priceAmountText == "-$10.0000")

        #expect(PriceViewModel(price: .mock(price: 123_456), currencyCode: currencyCode).priceAmountText == "$123,456.0000")
        #expect(PriceViewModel(price: .mock(price: 10_123_456), currencyCode: currencyCode).priceAmountText == "$10,123,456.0000")
    }

    @Test
    func priceAmountChangeText() {
        #expect(PriceViewModel(price: .mock(priceChangePercentage24h: 5), currencyCode: currencyCode).priceChangeText == "+5.0000%")
        #expect(PriceViewModel(price: .mock(priceChangePercentage24h: -5), currencyCode: currencyCode).priceChangeText == "-5.0000%")
    }
}
