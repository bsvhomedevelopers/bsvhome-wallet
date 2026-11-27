// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import struct BsvHomeCore.BsvHomeCurrencies
import protocol BsvHomeCore.BsvHomeCurrencyServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import Primitives

@Observable
@MainActor
public final class CurrencySceneViewModel {
    private let preferences: ObservablePreferences
    private let service: any BsvHomeCurrencyServiceProtocol

    var currency: Currency { preferences.currency }

    public init(
        preferences: ObservablePreferences,
        service: any BsvHomeCurrencyServiceProtocol,
    ) {
        self.preferences = preferences
        self.service = service
    }

    public var selectedCurrencyValue: String {
        "\(currencies.selected.flag) \(currency.rawValue)"
    }

    var title: String {
        Localized.Settings.currency
    }

    var list: [ListItemValueSection<CurrencyViewModel>] {
        let currencies = currencies
        let recommendedValues = currencies.recommended.map(CurrencyViewModel.init).map { ListItemValue(title: $0.title, value: $0) }
        let allValues = currencies.other.map(CurrencyViewModel.init).map { ListItemValue(title: $0.title, value: $0) }

        return [
            ListItemValueSection(
                section: Localized.Common.recommended,
                values: recommendedValues,
            ),
            ListItemValueSection(
                section: Localized.Common.all,
                values: allValues,
            ),
        ]
    }

    func setCurrency(_ currency: Currency) async throws {
        try await service.setCurrency(currency: currency.rawValue)
        preferences.reload()
    }
}

// MARK: - Private

extension CurrencySceneViewModel {
    private var localeCurrency: Currency? {
        Locale.current.currency.flatMap { Currency(rawValue: $0.identifier) }
    }

    private var currencies: BsvHomeCurrencies {
        service.currencies(locale: localeCurrency?.rawValue)
    }
}
