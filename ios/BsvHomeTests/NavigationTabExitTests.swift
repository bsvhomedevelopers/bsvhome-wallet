import Foundation
@testable import BsvHome
import BsvHomeCore
import BsvHomeCorePrimitivesTestKit
import Testing

@MainActor
struct NavigationTabExitTests {
    @Test
    func aDirectTabWriteCancelsTheActualScopeBeforeChangingTheSelection() async throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = preferences.navigation
        navigation.home.append("home-review")
        navigation.wallet.append("asset-details")
        navigation.settings.append("onboarding")
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        var observedTabs: [TabItem] = []
        navigation.observeTabExit(tab: .home) {
            observedTabs.append(navigation.selectedTab)
            scope.cancel()
        }

        navigation.selectedTab = .wallet

        #expect(observedTabs == [.home])
        #expect(navigation.selectedTab == .wallet)
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try scope.check() }
        #expect(try scope.outcome().submission == .cancelledBeforeSubmission)
        #expect(navigation.home.count == 1)
        #expect(navigation.wallet.count == 1)
        #expect(navigation.settings.count == 1)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test
    func selectingTheSameTabDoesNotRetireItsScope() async throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = preferences.navigation
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        var exits = 0
        navigation.observeTabExit(tab: .home) {
            exits += 1
            scope.cancel()
        }
        navigation.selectedTab = .home
        try scope.check()
        #expect(exits == 0)
        navigation.selectedTab = .settings
        #expect(exits == 1)
        navigation.selectedTab = .home
        navigation.selectedTab = .wallet
        #expect(exits == 1)
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try scope.check() }
    }

    @Test
    func removingOneObserverDoesNotRemoveAnotherTabsOwner() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = preferences.navigation
        var homeExits = 0
        var walletExits = 0
        let removed = navigation.observeTabExit(tab: .home) { homeExits += 1 }
        navigation.observeTabExit(tab: .wallet) { walletExits += 1 }
        navigation.removeTabExitObserver(removed)
        navigation.selectedTab = .wallet
        #expect(homeExits == 0)
        #expect(walletExits == 0)
        navigation.selectedTab = .settings
        #expect(homeExits == 0)
        #expect(walletExits == 1)
    }

    @Test
    func callbackCleanupUsesTheSnapshotAndNewRegistrationsWaitForTheNextExit() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = preferences.navigation
        var firstId: UUID?
        var secondId: UUID?
        var firstExits = 0
        var secondExits = 0
        var replacementExits = 0
        firstId = navigation.observeTabExit(tab: .home) {
            firstExits += 1
            if let secondId {
                navigation.removeTabExitObserver(secondId)
            }
            navigation.observeTabExit(tab: .home) { replacementExits += 1 }
        }
        secondId = navigation.observeTabExit(tab: .home) {
            secondExits += 1
            if let firstId {
                navigation.removeTabExitObserver(firstId)
            }
        }

        navigation.selectedTab = .wallet

        #expect(firstExits == 1)
        #expect(secondExits == 1)
        #expect(replacementExits == 0)
        navigation.selectedTab = .home
        navigation.selectedTab = .settings
        #expect(firstExits == 1)
        #expect(secondExits == 1)
        #expect(replacementExits == 1)
    }

    @Test
    func anOldCleanupTokenCannotCancelTheReplacementScopeOnTheSameTab() async throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = preferences.navigation
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let oldScope = try fixture.beginRetryFunding()
        let nextScope = try fixture.beginRetryFunding()
        var oldId: UUID?
        var oldExits = 0
        var nextExits = 0
        oldId = navigation.observeTabExit(tab: .home) {
            oldExits += 1
            oldScope.cancel()
            if let oldId {
                navigation.removeTabExitObserver(oldId)
            }
            navigation.observeTabExit(tab: .home) {
                nextExits += 1
                nextScope.cancel()
            }
        }
        navigation.selectedTab = .wallet
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try oldScope.check() }
        try nextScope.check()
        try navigation.removeTabExitObserver(#require(oldId))
        navigation.selectedTab = .home
        try nextScope.check()
        navigation.selectedTab = .settings
        #expect(oldExits == 1)
        #expect(nextExits == 1)
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try nextScope.check() }
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    private struct Preferences {
        let name = "NavigationTabExitTests.\(UUID().uuidString)"
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
