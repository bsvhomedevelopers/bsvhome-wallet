// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeSecurityService
@testable import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import Testing

struct BiometryAuthenticationServiceTests {
    @Test
    func requiresAuthenticationWhenKeychainUnreadable() {
        let keystorePassword = MockKeystorePassword(availableAuthentication: .none)
        let service = BiometryAuthenticationService(keystorePassword: keystorePassword, securityService: BsvHomeSecurityService())

        #expect(!service.requiresAuthentication)

        keystorePassword.getAuthenticationError = AnyError("keychain interaction not allowed")

        #expect(service.requiresAuthentication)
    }

    @Test
    func privacyLockStaysOnWhenTheKeychainIsUnreadable() {
        let keystorePassword = MockKeystorePassword(privacyLockStatus: .none)
        let service = BiometryAuthenticationService(keystorePassword: keystorePassword, securityService: BsvHomeSecurityService())

        #expect(!service.isPrivacyLockEnabled)

        keystorePassword.getPrivacyLockStatusError = AnyError("keychain interaction not allowed")

        #expect(service.isPrivacyLockEnabled)
    }

    @Test
    func requiresAuthenticationReflectsStoredAuthentication() {
        let keystorePassword = MockKeystorePassword(availableAuthentication: .biometrics)
        let service = BiometryAuthenticationService(keystorePassword: keystorePassword, securityService: BsvHomeSecurityService())

        #expect(service.requiresAuthentication)
    }
}
