import Foundation

enum BsvColdWalletAmount {
    static func satoshis(_ input: String, decimalSeparator: String = ".") -> UInt64? {
        guard decimalSeparator == "." || !input.contains(".") else { return nil }
        let normalized = decimalSeparator == "." ? input : input.replacingOccurrences(of: decimalSeparator, with: ".")
        let parts = normalized.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count <= 2, let whole = parts.first, !whole.isEmpty,
              whole.utf8.allSatisfy({ $0 >= 48 && $0 <= 57 }), let integer = UInt64(whole) else { return nil }
        let fraction = parts.count == 2 ? String(parts[1]) : ""
        guard parts.count == 1 || !fraction.isEmpty,
              fraction.count <= 8, fraction.utf8.allSatisfy({ $0 >= 48 && $0 <= 57 }) else { return nil }
        let scaled = integer.multipliedReportingOverflow(by: 100_000_000)
        guard !scaled.overflow, let fractional = UInt64(fraction.padding(toLength: 8, withPad: "0", startingAt: 0)) else { return nil }
        let total = scaled.partialValue.addingReportingOverflow(fractional)
        return !total.overflow && total.partialValue > 0 ? total.partialValue : nil
    }

    static func display(_ satoshis: UInt64) -> String {
        let whole = satoshis / 100_000_000
        let remainder = satoshis % 100_000_000
        guard remainder != 0 else { return String(whole) }
        var fraction = String(remainder)
        fraction = String(repeating: "0", count: 8 - fraction.count) + fraction
        while fraction.last == "0" {
            fraction.removeLast()
        }
        return String(whole) + "." + fraction
    }
}
