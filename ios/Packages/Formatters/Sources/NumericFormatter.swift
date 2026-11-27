// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation

public struct NumericFormatter: Sendable, Hashable {
    private let locale: Locale

    public init(locale: Locale = .current) {
        self.locale = locale
    }

    public func string(_ value: Double, symbol: String? = nil) -> String {
        let decimal = value.isFinite && !(value.isZero && value.sign == .minus)
            ? Decimal(string: String(value), locale: Locale(identifier: "en_US_POSIX"))
            : nil
        let number = if let decimal {
            decimal.formatted(.number.locale(locale).precision(.minimumFourPlaces))
        } else {
            value.formatted(.number.locale(locale).precision(.minimumFourPlaces))
        }
        guard let symbol else { return number }
        return "\(number) \(symbol)"
    }

    public func string(decimal value: Decimal, symbol: String? = nil) -> String {
        let number = value.formatted(.number.locale(locale).precision(.minimumFourPlaces))
        guard let symbol else { return number }
        return "\(number) \(symbol)"
    }

    public func string(_ value: BigInt, decimals: Int, symbol: String? = nil) -> String {
        ValueFormatter(locale: locale, style: .full).string(value, decimals: decimals, currency: symbol ?? "")
    }

    public func double(from amount: String) -> Double? {
        Decimal(string: amount, locale: locale)?.doubleValue
    }
}
