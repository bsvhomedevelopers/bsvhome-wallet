import Foundation
import class BsvHomeCore.BsvHomePreferencesService
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import LocalAuthentication
import Primitives
@testable import Settings
import Testing

@MainActor
struct SettingsEntryProducerTests {
    @Test
    func developerEntryIsOffForAFreshStore() {
        let core = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let model = SettingsViewModel(preferences: ObservablePreferences(preferencesService: core))
        #expect(!model.isDeveloperEnabled)
        #expect(!core.isDeveloperEnabled())
    }

    @Test
    func developerEntryUsesTheStoredFlagWithoutResettingIt() throws {
        let core = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        try core.setDeveloperEnabled(enabled: true)
        let preferences = ObservablePreferences(preferencesService: core)
        let model = SettingsViewModel(preferences: preferences)
        #expect(model.isDeveloperEnabled)
        #expect(core.isDeveloperEnabled())
        try core.setDeveloperEnabled(enabled: false)
        preferences.reload()
        #expect(!model.isDeveloperEnabled)
    }

    @Test
    func securityManagementKeepsTheExplicitSelectedPasswordDomain() {
        let core = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        var actions: [SecurityViewModel.PasswordManagement] = []
        let model = SecurityViewModel(service: StubBiometry(), preferences: ObservablePreferences(preferencesService: core), onManagePassword: { actions.append($0) })
        model.managePassword(.online)
        model.managePassword(.offline)
        #expect(actions == [.online, .offline])
    }

    @Test
    func securityWithoutAnActualManagementHandlerDoesNotOfferAnAction() {
        let core = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let model = SecurityViewModel(service: StubBiometry(), preferences: ObservablePreferences(preferencesService: core))
        #expect(model.passwordManagementActions.isEmpty)
        model.managePassword(.online)
        model.managePassword(.offline)
        #expect(model.passwordManagementActions.isEmpty)
    }

    private struct StubBiometry: BiometryAuthenticatable {
        let requiresAuthentication = false
        let availableAuthentication = KeystoreAuthentication.none
        let lockPeriod = LockPeriod.immediate
        let isPrivacyLockEnabled = false
        @MainActor func authenticate(context _: LAContext, reason _: String) async throws {}
        @MainActor func enableAuthentication(_: Bool, context _: LAContext, reason _: String) async throws {}
        func update(period _: LockPeriod) throws {}
        func togglePrivacyLock(enabled _: Bool) throws {}
        func shouldRelock(elapsedMilliseconds _: Int64) -> Bool {
            false
        }
    }
}
