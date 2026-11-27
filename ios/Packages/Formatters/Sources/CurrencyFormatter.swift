// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import Primitives

public enum CurrencyFormatterType: Sendable, Hashable {
    case currency
    case fiat
    case fiatNarrowSymbol
    case abbreviated
}

public struct CurrencyFormatter: Sendable, Hashable {
    private let locale: Locale
    private let type: CurrencyFormatterType
    public let currencyCode: String

    public init(
        type: CurrencyFormatterType = .currency,
        locale: Locale = Locale.current,
        currencyCode: String,
    ) {
        self.type = type
        self.locale = locale
        self.currencyCode = currencyCode
    }

    public var symbol: String {
        if usesNarrowSymbol {
            let value = 0.0.formatted(.currency(code: currencyCode).presentation(.narrow).locale(locale).attributed)
            return value.runs.filter { $0.numberSymbol == .currency }.map { String(value[$0.range].characters) }.joined()
        }
        let formatter = NumberFormatter()
        formatter.locale = locale
        formatter.numberStyle = .currency
        formatter.currencyCode = currencyCode
        return formatter.currencySymbol
    }

    public func string(_ value: Double) -> String {
        if let decimal = decimal(from: value) {
            return string(decimal: decimal)
        }

        return switch type {
        case .currency, .fiat, .fiatNarrowSymbol: currencyString(value)
        case .abbreviated: abbreviatedFormatter.string(from: value, currency: currencyCode) ?? currencyString(value)
        }
    }

    public func string(decimal value: Decimal) -> String {
        switch type {
        case .currency, .fiat, .fiatNarrowSymbol: currencyString(value)
        case .abbreviated: abbreviatedFormatter.string(from: value, currency: currencyCode) ?? currencyString(value)
        }
    }

    public func balanceString(_ value: Double) -> String {
        guard let decimal = decimal(from: value) else {
            return string(value)
        }

        return balanceString(decimal: decimal)
    }

    public func balanceString(decimal value: Decimal) -> String {
        let precision = NumberFormatStyleConfiguration.Precision.fractionLength(4 ... 4)
        switch type {
        case .currency, .fiat, .fiatNarrowSymbol:
            return currencyString(value, precision: precision)
        case .abbreviated:
            return abbreviatedFormatter.string(from: value, currency: currencyCode, precision: precision)
                ?? currencyString(value, precision: precision)
        }
    }

    public func signedNumber(_ value: Double) -> String {
        if let decimal = decimal(from: value) {
            return signedNumber(decimal: decimal)
        }

        return value.formatted(.number.locale(locale).precision(.minimumFourPlaces).sign(strategy: .always()))
    }

    public func signedNumber(decimal value: Decimal) -> String {
        value.formatted(.number.locale(locale).precision(.minimumFourPlaces).sign(strategy: .always()))
    }
}

// MARK: - Private

private extension CurrencyFormatter {
    var usesNarrowSymbol: Bool {
        currencyCode == Currency.usd.rawValue || type == .fiatNarrowSymbol
    }

    var abbreviatedFormatter: AbbreviatedFormatter {
        AbbreviatedFormatter(locale: locale)
    }

    func currencyString(_ value: Double) -> String {
        value.formatted(.currency(code: currencyCode).presentation(usesNarrowSymbol ? .narrow : .standard).locale(locale).precision(.minimumFourPlaces))
    }

    func currencyString(_ value: Decimal, precision: NumberFormatStyleConfiguration.Precision = .minimumFourPlaces) -> String {
        value.formatted(
            .currency(code: currencyCode)
                .presentation(usesNarrowSymbol ? .narrow : .standard)
                .locale(locale)
                .precision(precision)
                .rounded(rule: .towardZero),
        )
    }

    func decimal(from value: Double) -> Decimal? {
        guard value.isFinite, !(value.isZero && value.sign == .minus) else { return nil }
        return Decimal(string: String(value), locale: Locale(identifier: "en_US_POSIX"))
    }
}
