// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import Primitives

public struct ValueFormatter: Sendable {
    public enum Style: Sendable {
        case full, short, auto
    }

    private enum FractionDigits {
        case atLeastFour
        case exactlyFour

        var precision: NumberFormatStyleConfiguration.Precision {
            switch self {
            case .atLeastFour: .minimumFourPlaces
            case .exactlyFour: .fractionLength(4 ... 4)
            }
        }

        var maximum: Int {
            switch self {
            case .atLeastFour: .max
            case .exactlyFour: 4
            }
        }
    }

    private static let decimalSignificantDigitLimit = 38
    private static let canonicalLocale = Locale(identifier: "en_US_POSIX")
    private static let canonicalFormatter = BigNumberFormatter(locale: canonicalLocale, groupingSeparator: "")

    private let locale: Locale
    private let style: Style
    private let abbreviationThreshold: Decimal

    public init(
        locale: Locale = .current,
        style: Style,
        abbreviationThreshold: Decimal = defaultAbbreviationThreshold,
    ) {
        self.locale = locale
        self.style = style
        self.abbreviationThreshold = abbreviationThreshold
    }

    public func string(_ value: BigInt, asset: Asset) -> String {
        string(value, decimals: asset.decimals.asInt, currency: asset.symbol)
    }

    public func string(_ value: BigInt, decimals: Int, currency: String = "") -> String {
        formattedString(value, decimals: decimals, currency: currency, fractionDigits: .atLeastFour)
    }

    public func balanceString(_ value: BigInt, asset: Asset, currency: String = "") -> String {
        let fractionDigits: FractionDigits = asset.symbol == "USDT" ? .exactlyFour : .atLeastFour
        return formattedString(value, decimals: asset.decimals.asInt, currency: currency, fractionDigits: fractionDigits)
    }

    private func formattedString(_ value: BigInt, decimals: Int, currency: String, fractionDigits: FractionDigits) -> String {
        if decimals > Self.decimalSignificantDigitLimit || value.magnitude.description.count > Self.decimalSignificantDigitLimit {
            let exact = exactAbbreviatedString(value, decimals: decimals, fractionDigits: fractionDigits)
                ?? BigNumberFormatter(
                    locale: locale,
                    minimumFractionDigits: 4,
                    maximumFractionDigits: fractionDigits.maximum,
                ).string(from: value, decimals: decimals)
            return appendingCurrency(exact, currency: currency)
        }
        let decimal = BigNumberFormatter.standard.decimal(from: value, decimals: decimals)
        if style == .short, let decimal, abs(decimal) >= abbreviationThreshold,
           let abbreviated = abbreviatedFormatter.string(from: decimal, precision: fractionDigits.precision)
        {
            return appendingCurrency(abbreviated, currency: currency)
        }
        guard let decimal else {
            return ""
        }
        return appendingCurrency(decimal.formatted(formatStyle(precision: fractionDigits.precision)), currency: currency)
    }

    public func inputString(_ value: BigInt, decimals: Int) -> String {
        guard let decimal = BigNumberFormatter.standard.decimal(from: value, decimals: decimals) else {
            return ""
        }
        return decimal.formatted(formatStyle(precision: .full))
    }

    public func inputNumber(from string: String, decimals: Int) throws -> BigInt {
        try BigNumberFormatter.standard.number(from: plainInputNumber(string), decimals: decimals)
    }

    public func plainInputNumber(_ string: String) -> String {
        NumberInputNormalizer.normalize(string, locale: locale)
    }

    public func double(from number: BigInt, decimals: Int) throws -> Double {
        guard let result = BigNumberFormatter.standard.double(from: number, decimals: decimals) else {
            throw AnyError("unknown \(number) number")
        }
        return result
    }

    public func decimal(from number: BigInt, decimals: Int) throws -> Decimal {
        let exactString = Self.canonicalFormatter.string(from: number, decimals: decimals)
        guard let decimal = Decimal(string: exactString, locale: Self.canonicalLocale) else {
            throw AnyError("unable to represent atomic amount as Decimal")
        }
        let formatted = decimal.formatted(
            Decimal.FormatStyle()
                .locale(Self.canonicalLocale)
                .grouping(.never)
                .precision(.fractionLength(0 ... 324)),
        )
        guard formatted == exactString else {
            throw AnyError("atomic amount exceeds Decimal precision")
        }
        return decimal
    }

    public func number(amount: String) throws -> Decimal {
        try number(amount: amount, locale: locale)
    }

    func number(amount: String, locale: Locale) throws -> Decimal {
        guard let decimal = Decimal(string: amount, locale: locale) else {
            throw AnyError("unknown \(amount) decimal")
        }
        return decimal
    }
}

private extension ValueFormatter {
    var abbreviatedFormatter: AbbreviatedFormatter {
        AbbreviatedFormatter(locale: locale, threshold: abbreviationThreshold)
    }

    private func exactAbbreviatedString(_ value: BigInt, decimals: Int, fractionDigits: FractionDigits) -> String? {
        guard style == .short, #available(iOS 18, *) else {
            return nil
        }

        let thresholdString = abbreviationThreshold.formatted(
            Decimal.FormatStyle()
                .locale(Self.canonicalLocale)
                .grouping(.never)
                .precision(.fractionLength(0 ... 324)),
        )
        let thresholdDecimals = thresholdString.firstIndex(of: ".").map {
            thresholdString.distance(from: thresholdString.index(after: $0), to: thresholdString.endIndex)
        } ?? 0
        guard let thresholdUnits = BigInt(thresholdString.replacingOccurrences(of: ".", with: "")) else {
            return nil
        }

        let magnitude = BigInt(sign: .plus, magnitude: value.magnitude)
        let commonDecimals = max(decimals, thresholdDecimals)
        let scaledMagnitude = magnitude * BigInt(10).power(commonDecimals - decimals)
        let scaledThreshold = thresholdUnits * BigInt(10).power(commonDecimals - thresholdDecimals)
        guard scaledMagnitude >= scaledThreshold else {
            return nil
        }

        for (shift, suffix) in [(9, "B"), (6, "M"), (3, "K")] {
            if magnitude >= BigInt(10).power(decimals + shift) {
                let scaled = BigNumberFormatter(
                    locale: locale,
                    minimumFractionDigits: 4,
                    maximumFractionDigits: fractionDigits.maximum,
                ).string(from: value, decimals: decimals + shift)
                return "\(scaled)\(suffix)"
            }
        }
        return nil
    }

    func formatStyle(precision: NumberFormatStyleConfiguration.Precision) -> Decimal.FormatStyle {
        Decimal.FormatStyle()
            .locale(locale)
            .grouping(.automatic)
            .rounded(rule: .towardZero)
            .precision(precision)
    }

    func appendingCurrency(_ value: String, currency: String) -> String {
        currency.isEmpty ? value : "\(value) \(currency)"
    }
}
