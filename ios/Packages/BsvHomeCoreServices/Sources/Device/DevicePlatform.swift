// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeDeviceKeyService
import struct BsvHomeCore.BsvHomeDeviceInfo
import protocol BsvHomeCore.BsvHomeDevicePlatform
import protocol BsvHomeCore.BsvHomePreferencesServiceProtocol
import BsvHomeCorePrimitives
import Primitives
import UIKit
import UserNotifications

public final class BsvHomeCoreDevicePlatform: BsvHomeDevicePlatform, @unchecked Sendable {
    private let preferencesService: any BsvHomePreferencesServiceProtocol
    private let deviceKeyService: BsvHomeDeviceKeyService
    private let securePreferences: SecurePreferences
    private let os: String
    private let model: String

    @MainActor
    public init(
        preferencesService: any BsvHomePreferencesServiceProtocol,
        deviceKeyService: BsvHomeDeviceKeyService,
        securePreferences: SecurePreferences = SecurePreferences(),
    ) {
        self.preferencesService = preferencesService
        self.deviceKeyService = deviceKeyService
        self.securePreferences = securePreferences
        os = UIDevice.current.osName
        model = UIDevice.current.modelName
    }

    public func deviceId() async throws -> String {
        try deviceKeyService.deviceId()
    }

    public func deviceInfo() async throws -> BsvHomeDeviceInfo {
        BsvHomeDeviceInfo(
            platform: Platform.ios.map(),
            platformStore: PlatformStore.current.map(),
            os: os,
            model: model,
            version: Bundle.main.releaseVersionNumber,
            localeIdentifier: Locale.current.identifier(.bcp47),
        )
    }

    public func pushToken() async throws -> String {
        try securePreferences.get(key: .deviceToken) ?? .empty
    }

    public func isPushEnabled() async throws -> Bool {
        guard preferencesService.isPushNotificationsEnabled() else {
            return false
        }
        return await UNUserNotificationCenter.current().notificationSettings().authorizationStatus.isAuthorized
    }

    public func getCurrency() async throws -> String {
        preferencesService.getCurrency()
    }
}
