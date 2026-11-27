import Formatters
import Foundation
import Localization
import Primitives

struct WalletBalanceViewModel {
    let primaryValue: Double?
    let secondaryValue: Double?
    let currency: Currency
    let locale: Locale
    let secondaryTitle: String
    let secondaryDecimalValue: Decimal?

    init(
        primaryValue: Double?,
        secondaryValue: Double?,
        currency: Currency = .usd,
        locale: Locale = .current,
        secondaryTitle: String? = nil,
        secondaryDecimalValue: Decimal? = nil,
    ) {
        self.primaryValue = primaryValue
        self.secondaryValue = secondaryValue
        self.currency = currency
        self.locale = locale
        self.secondaryTitle = secondaryTitle ?? currency.rawValue
        self.secondaryDecimalValue = secondaryDecimalValue
    }

    var title: String {
        "\(Localized.wowpayTotalAssets) (\(currency.rawValue))"
    }

    var value: String {
        formatted(primaryValue, type: WalletBalanceConfiguration.primaryFormatting)
    }

    var secondaryAmount: String {
        let formatter = CurrencyFormatter(type: .fiat, locale: locale, currencyCode: currency.rawValue)
        let amount = secondaryDecimalValue.map { formatter.balanceString(decimal: $0) }
            ?? secondaryValue.map(formatter.balanceString)
            ?? "\(formatter.symbol) --"
        return "≈ \(amount)"
    }

    private func formatted(_ amount: Double?, type: CurrencyFormatterType) -> String {
        let formatter = CurrencyFormatter(type: type, locale: locale, currencyCode: currency.rawValue)
        return amount.map(formatter.balanceString) ?? "\(formatter.symbol) --"
    }
}
