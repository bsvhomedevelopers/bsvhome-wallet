import BigInt
import Formatters
import Foundation
import enum BsvHomeCore.BsvHomeBsvMultisigError

struct BsvMultisigAmountFormatter {
    private let locale: Locale
    private let formatter = BigNumberFormatter(minimumFractionDigits: 8, maximumFractionDigits: 8)

    init(locale: Locale = .current) {
        self.locale = locale
    }

    func amount(_ satoshis: String?, symbol: Bool = false) -> String {
        guard let satoshis, let value = BigInt(satoshis) else { return "—" }
        let text = formatter.string(from: value, decimals: 8)
        return symbol ? "\(text) BSV" : text
    }

    func input(_ canonical: String) -> String {
        canonical.replacingOccurrences(of: ".", with: locale.decimalSeparator ?? ".")
    }

    func encoded(_ text: String) throws -> String {
        let separator = locale.decimalSeparator ?? "."
        if let grouping = locale.groupingSeparator, !grouping.isEmpty, text.contains(grouping) {
            throw BsvHomeBsvMultisigError.Api(code: "INVALID_AMOUNT")
        }
        if separator != ".", text.contains(separator), text.contains(".") {
            throw BsvHomeBsvMultisigError.Api(code: "INVALID_AMOUNT")
        }
        return text.replacingOccurrences(of: separator, with: ".")
    }
}
