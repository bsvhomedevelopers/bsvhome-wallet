// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

public struct PreferencesScene: View {
    @Environment(\.openURL) private var openURL

    @State private var model: PreferencesViewModel

    public init(model: PreferencesViewModel) {
        _model = State(initialValue: model)
    }

    public var body: some View {
        List {
            Section {
                NavigationLink(value: Scenes.Currency()) {
                    ListItemView(
                        title: model.currencyTitle,
                        subtitle: model.currencyValue,
                        imageStyle: .settings(assetImage: model.currencyImage),
                    )
                }

                NavigationCustomLink(
                    with: ListItemView(
                        title: model.languageTitle,
                        subtitle: model.languageValue,
                        imageStyle: .settings(assetImage: model.languageImage),
                    ),
                    action: onSelectLanguage,
                )

                NavigationLink(value: Scenes.Appearance()) {
                    ListItemView(
                        title: model.appearanceTitle,
                        subtitle: model.appearanceValue,
                        imageStyle: .settings(assetImage: model.appearanceImage),
                    )
                }
                NavigationLink(value: Scenes.Chains()) {
                    ListItemView(title: Localized.Settings.Networks.title, imageStyle: .settings(assetImage: .image(Images.Settings.networks)))
                }
                .accessibilityIdentifier("settingsPreferencesNetworks")
                NavigationLink(value: Scenes.Contacts()) {
                    ListItemView(title: Localized.Contacts.title, imageStyle: .settings(assetImage: .image(Images.Settings.contacts)))
                }
                .accessibilityIdentifier("settingsPreferencesContacts")
            }
            .listRowInsets(.assetListRowInsets)
            .listRowBackground(Colors.WowPay.surface)
        }
        .contentMargins(.top, .scene.top, for: .scrollContent)
        .listSectionSpacing(.compact)
        .scrollContentBackground(.hidden)
        .wowPayBackground(.solid)
        .navigationTitle(model.title)
    }
}

// MARK: - Actions

extension PreferencesScene {
    private func onSelectLanguage() {
        if let settingsURL = URL(string: UIApplication.openSettingsURLString) {
            openURL(settingsURL)
        }
    }
}
