// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import Primitives

public final class BigNumberFormatter: Sendable {
    public static let standard: BigNumberFormatter = .init(locale: Locale(identifier: "en_US"))

    let locale: Locale
    let decimalSeparator: String

    private let minimumFractionDigits: Int
    private let maximumFractionDigits: Int
    private let groupingSeparator: String
    private let groupingSize: Int
    private let secondaryGroupingSize: Int

    public init(
        locale: Locale = .current,
        minimumFractionDigits: Int = 0,
        maximumFractionDigits: Int = Int.max,
        groupingSeparator: String? = nil,
    ) {
        self.locale = locale
        self.minimumFractionDigits = minimumFractionDigits
        self.maximumFractionDigits = maximumFractionDigits
        decimalSeparator = locale.decimalSeparator ?? "."
        self.groupingSeparator = groupingSeparator ?? (locale.groupingSeparator ?? ",")
        let groupingFormatter = NumberFormatter()
        groupingFormatter.locale = locale
        groupingFormatter.numberStyle = .decimal
        groupingSize = groupingFormatter.groupingSize
        secondaryGroupingSize = groupingFormatter.secondaryGroupingSize > 0 ? groupingFormatter.secondaryGroupingSize : groupingFormatter.groupingSize
    }

    public func number(from string: String, decimals: Int) throws -> BigInt {
        try number(from: string, decimals: decimals, truncating: true)
    }

    public func exactNumber(from string: String, decimals: Int) throws -> BigInt {
        try number(from: string, decimals: decimals, truncating: false)
    }

    private func number(from string: String, decimals: Int, truncating: Bool) throws -> BigInt {
        guard let decimalIndex = string.firstIndex(where: { String($0) == decimalSeparator }) else {
            if let value = BigInt(string).flatMap({ $0 * BigInt(10).power(decimals) }) {
                return value
            }
            throw AnyError("unable to get number for \(string)")
        }

        let fractionalDigits = string.distance(from: string.index(after: decimalIndex), to: string.endIndex)

        var fullString = string
        if fractionalDigits > decimals {
            let integerPartString = string[..<string.index(after: decimalIndex)]
            let fractionPartString = string[string.index(after: decimalIndex)...]

            let endIndex = fractionPartString.index(fractionPartString.startIndex, offsetBy: decimals)
            let trimmedFractionString = fractionPartString[..<endIndex]

            guard truncating || fractionPartString[endIndex...].allSatisfy({ $0 == "0" }) else {
                throw AnyError("\(string) carries more than \(decimals) decimals")
            }
            fullString = String(integerPartString + trimmedFractionString)
        }

        fullString.remove(at: decimalIndex)

        guard let number = BigInt(fullString) else {
            throw AnyError("unable to get number for \(fullString)")
        }

        if fractionalDigits < decimals {
            return number * BigInt(10).power(decimals - fractionalDigits)
        } else {
            return number
        }
    }

    public func number(from value: Int, decimals: Int) -> BigInt {
        BigInt(value) * BigInt(10).power(decimals)
    }

    public func string(from number: BigInt, decimals: Int) -> String {
        let dividend = BigInt(10).power(decimals)
        let (integerPart, remainder) = number.quotientAndRemainder(dividingBy: dividend)
        let integerString = integerPart.isZero && number.sign == .minus ? "-0" : integerString(from: integerPart)
        let fractionalString = fractionalString(from: BigInt(sign: .plus, magnitude: remainder.magnitude), decimals: decimals)
        if fractionalString.isEmpty {
            return integerString
        }
        return "\(integerString)\(decimalSeparator)\(fractionalString)"
    }

    func decimal(from number: BigInt, decimals: Int) -> Decimal? {
        let dividend = BigInt(10).power(decimals)
        let (integerPart, remainder) = number.quotientAndRemainder(dividingBy: dividend)
        let integerString = integerPart.isZero && number.sign == .minus ? "-0" : integerPart.description
        let fractionalString = fractionalString(from: BigInt(sign: .plus, magnitude: remainder.magnitude), decimals: decimals)
        if fractionalString.isEmpty {
            return Decimal(string: integerString)
        }
        return Decimal(string: "\(integerString)\(decimalSeparator)\(fractionalString)", locale: locale)
    }

    func double(from number: BigInt, decimals: Int) -> Double? {
        guard let decimal = decimal(from: number, decimals: decimals) else {
            return .none
        }
        return decimal.doubleValue
    }
}

// MARK: - Private

extension BigNumberFormatter {
    private func integerString(from bigInt: BigInt) -> String {
        var resultString = bigInt.description
        let isNegative = bigInt.sign == .minus
        let firstDigitOffset = isNegative ? 1 : 0
        guard groupingSize > 0 else {
            return resultString
        }

        var offset = resultString.count - groupingSize
        while offset > firstDigitOffset {
            let index = resultString.index(resultString.startIndex, offsetBy: offset)
            resultString.insert(contentsOf: groupingSeparator, at: index)
            offset -= secondaryGroupingSize
        }

        return resultString
    }

    private func fractionalString(from number: BigInt, decimals: Int) -> String {
        guard decimals > 0, !number.isZero else {
            return String(repeating: .zero, count: minimumFractionDigits)
        }

        let digits = number.description
        let leadingZeros = String(repeating: .zero, count: max(0, decimals - digits.count))
        let fractionalDigits = String((leadingZeros + digits).prefix(maximumFractionDigits))
        let trimmedDigits = String(fractionalDigits.reversed().drop(while: { $0 == "0" }).reversed())
        return trimmedDigits + String(repeating: .zero, count: max(0, minimumFractionDigits - trimmedDigits.count))
    }
}
