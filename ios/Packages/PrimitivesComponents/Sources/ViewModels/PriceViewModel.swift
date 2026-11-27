// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Components
import Formatters
import Foundation
import Primitives
import Style
import SwiftUI

public struct PriceViewModel: Sendable {
    public let price: Price?

    private let currencyFormatter: CurrencyFormatter
    static let percentFormatter = PercentFormatter.signed

    public init(
        price: Price?,
        currencyCode: String,
        currencyFormatterType: CurrencyFormatterType = .currency,
    ) {
        self.price = price
        currencyFormatter = CurrencyFormatter(type: currencyFormatterType, currencyCode: currencyCode)
    }

    public var isPriceAvailable: Bool {
        guard let price else { return false }
        return price.price != 0
    }

    public var priceAmountText: String {
        guard let price else { return "" }
        return currencyFormatter.string(price.price)
    }

    private var priceChange: Double? {
        price?.priceChangePercentage24h ?? .none
    }

    public var priceChangeText: String {
        guard let priceChange else { return "" }
        return Self.percentFormatter.string(priceChange)
    }

    public var priceChangeAmountText: String {
        guard let price, price.price != 0, price.priceChangePercentage24h > -100 else { return "—" }
        if let currentPrice = Decimal(string: String(price.price), locale: Locale(identifier: "en_US_POSIX")),
           let percentChange = Decimal(string: String(price.priceChangePercentage24h), locale: Locale(identifier: "en_US_POSIX"))
        {
            let previousPrice = currentPrice / (1 + percentChange / 100)
            return currencyFormatter.signedNumber((currentPrice - previousPrice).doubleValue)
        }
        let previousPrice = price.price / (1 + price.priceChangePercentage24h / 100)
        return currencyFormatter.signedNumber(price.price - previousPrice)
    }

    public var priceChangeTextColor: Color {
        Self.priceChangeTextColor(value: priceChange)
    }

    public static func priceChangeTextColor(value: Double?) -> Color {
        guard let value else { return Colors.gray }
        return PriceChangeColor.color(for: value)
    }

    public var priceChangeTextBackgroundColor: Color {
        if priceChange == 0 {
            return Colors.grayVeryLight
        } else if priceChange ?? 0 > 0 {
            return Colors.greenLight
        }
        return Colors.redLight
    }

    public func fiatAmountText(amount: Double) -> String {
        currencyFormatter.string(amount)
    }

    public func fiatAmountText(value: BigInt, decimals: Int) -> String? {
        guard let price,
              let rate = Decimal(string: String(price.price), locale: Locale(identifier: "en_US_POSIX"))
        else { return nil }
        let rateText = rate.formatted(
            Decimal.FormatStyle()
                .locale(Locale(identifier: "en_US_POSIX"))
                .grouping(.never)
                .precision(.fractionLength(0 ... 324)),
        )
        let rateDecimals = rateText.firstIndex(of: ".").map {
            rateText.distance(from: rateText.index(after: $0), to: rateText.endIndex)
        } ?? 0
        guard let rateUnits = BigInt(rateText.replacingOccurrences(of: ".", with: "")),
              let fiatValue = try? ValueFormatter.full.decimal(from: value * rateUnits, decimals: decimals + rateDecimals)
        else { return nil }
        return currencyFormatter.string(decimal: fiatValue)
    }
}
