// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import BsvHomeCoreServices
import Localization
import Observation
import Style

@Observable
@MainActor
public final class PreferencesViewModel {
    private let currencyModel: CurrencySceneViewModel
    private let preferences: ObservablePreferences

    public init(
        currencyModel: CurrencySceneViewModel,
        preferences: ObservablePreferences,
    ) {
        self.currencyModel = currencyModel
        self.preferences = preferences
    }

    var title: String {
        Localized.Settings.Preferences.title
    }

    var currencyTitle: String { Localized.Settings.currency }

    var currencyValue: String { currencyModel.selectedCurrencyValue }

    var currencyImage: AssetImage { .image(Images.Settings.currency) }

    var languageTitle: String {
        Localized.Settings.language
    }

    var languageValue: String {
        guard let code = Locale.current.language.languageCode?.identifier else {
            return ""
        }
        return Locale.current.localizedString(forLanguageCode: code)?.capitalized ?? ""
    }

    var languageImage: AssetImage {
        AssetImage.image(Images.Settings.language)
    }

    var appearanceTitle: String {
        Localized.Settings.appearanceTitle
    }

    var appearanceImage: AssetImage {
        AssetImage.image(Images.Settings.appearance)
    }

    var appearanceValue: String {
        preferences.appearance.title
    }
}
