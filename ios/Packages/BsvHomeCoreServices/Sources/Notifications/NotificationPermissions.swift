// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeNotificationPermissions

public final class BsvHomeCoreNotificationPermissions: BsvHomeNotificationPermissions, Sendable {
    private let service: PushNotificationEnablerService

    public init(service: PushNotificationEnablerService) {
        self.service = service
    }

    public func requestPermissionsOrOpenSettings() async throws -> Bool {
        try await service.requestPermissionsOrOpenSettings()
    }
}
