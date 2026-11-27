// Copyright (c). Gem Wallet. All rights reserved.

import BsvHomeCoreServices
import Localization
import Observation
import Primitives
import Store

@Observable
@MainActor
public final class SettingsViewModel {
    public let walletsQuery = ObservableQuery(WalletsRequest(isPinned: .none), initialValue: [Wallet]())

    private let preferences: ObservablePreferences

    public init(preferences: ObservablePreferences) {
        self.preferences = preferences
    }

    var isDeveloperEnabled: Bool { preferences.isDeveloperEnabled }

    var walletsTitle: String {
        Localized.Wallets.title
    }

    var walletsValue: String {
        "\(walletsQuery.value.count)"
    }
}
