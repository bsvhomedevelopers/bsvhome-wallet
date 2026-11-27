// Copyright (c). Gem Wallet. All rights reserved.

import class BsvHomeCore.BsvHomePreferencesService
import protocol BsvHomeCore.BsvHomePreferencesServiceProtocol
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices

public extension ObservablePreferences {
    static func mock(
        preferencesService: any BsvHomePreferencesServiceProtocol = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock()),
        isPerpetualEnabled: Bool = true,
    ) -> ObservablePreferences {
        let observable = ObservablePreferences(preferencesService: preferencesService)
        observable.isPerpetualEnabled = isPerpetualEnabled
        return observable
    }
}
