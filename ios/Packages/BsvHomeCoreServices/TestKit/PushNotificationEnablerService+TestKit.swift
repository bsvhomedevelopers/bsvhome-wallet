// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomePreferencesServiceProtocol
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices

public extension PushNotificationEnablerService {
    static func mock(preferencesService: any BsvHomePreferencesServiceProtocol = BsvHomePreferencesServiceMock()) -> Self {
        PushNotificationEnablerService(preferencesService: preferencesService)
    }
}
