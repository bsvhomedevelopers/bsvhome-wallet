import Foundation
@testable import BsvHome
import BsvHomeCore
import BsvHomeCorePrimitivesTestKit
import Testing

@MainActor
struct NavigationPersistentTabExitTests {
    @Test
    func repeatedProgrammaticTabChangesCancelEachCurrentScopeBeforeCommit() async throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = preferences.navigation
        navigation.home.append("home-review")
        navigation.wallet.append("asset-details")
        navigation.settings.append("backup-flow")
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let first = try fixture.beginRetryFunding()
        let second = try fixture.beginRetryFunding()
        let third = try fixture.beginRetryFunding()
        var current = first
        var observedTabs: [TabItem] = []
        navigation.registerTabExitOwner {
            observedTabs.append(navigation.selectedTab)
            current.cancel()
        }

        navigation.selectedTab = .wallet
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try first.check() }
        current = second
        navigation.selectedTab = .settings
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try second.check() }
        current = third
        navigation.selectedTab = .home
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try third.check() }
        #expect(observedTabs == [.home, .wallet, .settings])
        #expect(navigation.home.count == 1)
        #expect(navigation.wallet.count == 1)
        #expect(navigation.settings.count == 1)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test
    func sameTabAssignmentsAndInternalPushesPreserveTheScope() async throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = preferences.navigation
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        var exits = 0
        navigation.registerTabExitOwner {
            exits += 1
            scope.cancel()
        }
        navigation.home.append("review")
        navigation.home.append("internal-page")
        navigation.home.removeLast()
        navigation.selectedTab = .home
        try scope.check()
        #expect(exits == 0)
        navigation.selectedTab = .wallet
        #expect(exits == 1)
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try scope.check() }
        #expect(navigation.home.count == 1)
    }

    @Test
    func oldRootCleanupCannotRemoveOrCancelTheReplacementOwner() async throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = preferences.navigation
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        var oldExits = 0
        var newExits = 0
        let oldId = navigation.registerTabExitOwner { oldExits += 1 }
        let newId = navigation.registerTabExitOwner {
            newExits += 1
            scope.cancel()
        }
        navigation.removeTabExitOwner(oldId)
        try scope.check()
        navigation.selectedTab = .wallet
        #expect(oldExits == 0)
        #expect(newExits == 1)
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try scope.check() }
        navigation.removeTabExitOwner(oldId)
        navigation.selectedTab = .settings
        #expect(newExits == 2)
        navigation.removeTabExitOwner(newId)
        navigation.selectedTab = .home
        #expect(newExits == 2)
    }

    @Test
    func replacementRegisteredInsideTheExitWaitsUntilTheNextActualTabChange() async throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = preferences.navigation
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let oldScope = try fixture.beginRetryFunding()
        let nextScope = try fixture.beginRetryFunding()
        var oldId: UUID?
        var oldExits = 0
        var nextExits = 0
        oldId = navigation.registerTabExitOwner {
            oldExits += 1
            oldScope.cancel()
            if let oldId {
                navigation.removeTabExitOwner(oldId)
            }
            navigation.registerTabExitOwner {
                nextExits += 1
                nextScope.cancel()
            }
        }
        navigation.selectedTab = .wallet
        #expect(oldExits == 1)
        #expect(nextExits == 0)
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try oldScope.check() }
        try nextScope.check()
        try navigation.removeTabExitOwner(#require(oldId))
        navigation.selectedTab = .settings
        #expect(oldExits == 1)
        #expect(nextExits == 1)
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try nextScope.check() }
    }

    @Test
    func removingTheRootOwnerLeavesScopedFinancialExitObserversIntact() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = preferences.navigation
        var rootExits = 0
        var scopedExits = 0
        let owner = navigation.registerTabExitOwner { rootExits += 1 }
        navigation.observeTabExit(tab: .home) { scopedExits += 1 }
        navigation.removeTabExitOwner(owner)
        navigation.selectedTab = .wallet
        navigation.selectedTab = .settings
        #expect(rootExits == 0)
        #expect(scopedExits == 1)
    }

    private struct Preferences {
        let name = "NavigationPersistentTabExitTests.\(UUID().uuidString)"
        let defaults: UserDefaults
        let navigation: NavigationStateManager

        init() throws {
            defaults = try #require(UserDefaults(suiteName: name))
            navigation = NavigationStateManager(defaults: defaults, portalOrigin: "")
        }

        func clear() {
            defaults.removePersistentDomain(forName: name)
        }
    }
}
