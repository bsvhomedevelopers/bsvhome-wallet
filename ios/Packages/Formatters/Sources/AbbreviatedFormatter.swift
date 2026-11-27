// Copyright (c). Gem Wallet. All rights reserved.

import Foundation

public let defaultAbbreviationThreshold: Decimal = 100_000.0

public struct AbbreviatedFormatter {
    private let locale: Locale
    private let threshold: Decimal

    public init(
        locale: Locale = .current,
        threshold: Decimal = defaultAbbreviationThreshold,
    ) {
        self.locale = locale
        self.threshold = threshold
    }

    public func string(from double: Double) -> String? {
        guard let decimal = decimal(from: double) else { return nil }
        return string(from: decimal)
    }

    public func string(from double: Double, currency: String) -> String? {
        guard let decimal = decimal(from: double) else { return nil }
        return string(from: decimal, currency: currency)
    }

    func string(from decimal: Decimal) -> String? {
        string(from: decimal, precision: .minimumFourPlaces)
    }

    func string(from decimal: Decimal, precision: NumberFormatStyleConfiguration.Precision) -> String? {
        guard abs(decimal) >= threshold, #available(iOS 18, *) else {
            return nil
        }

        return decimal.formatted(
            .number
                .notation(.compactName)
                .locale(locale)
                .precision(precision)
                .rounded(rule: .towardZero),
        )
    }

    func string(from decimal: Decimal, currency: String) -> String? {
        string(from: decimal, currency: currency, precision: .minimumFourPlaces)
    }

    func string(from decimal: Decimal, currency: String, precision: NumberFormatStyleConfiguration.Precision) -> String? {
        guard abs(decimal) >= threshold, #available(iOS 18, *) else {
            return nil
        }

        return decimal.formatted(
            .currency(code: currency)
                .presentation(currency == "USD" ? .narrow : .standard)
                .notation(.compactName)
                .locale(locale)
                .precision(precision)
                .rounded(rule: .towardZero),
        )
    }

    private func decimal(from double: Double) -> Decimal? {
        guard double.isFinite else { return nil }
        return Decimal(string: String(double), locale: Locale(identifier: "en_US_POSIX"))
    }
}
