import Components
import Foundation
@testable import BsvHome
import BsvHomeCorePrimitives
import Observation
import Primitives
import SwiftUI
import Testing
import UIKit

@MainActor
struct WalletPresentationTests {
    @Test
    func restoresAnOverviewWithoutInventingARemoteWalletId() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let selection = WalletPresentation.collectiveOverview(signerWalletId: .multicoin(address: "public-signer"))
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        navigation.selectWalletPresentation(selection)

        #expect(NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin).walletPresentation == selection)
        #expect(NavigationStateManager(defaults: preferences.defaults, portalOrigin: "https://other-portal.example").walletPresentation == .local)
    }

    @Test
    func changingPresentationCancelsSettingsBeforeDiscardingItsPath() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        let selection = collective("team")
        navigation.selectWalletPresentation(selection)
        navigation.settings.append("active-onboarding")
        var cancellationDepths: [Int] = []
        navigation.onSettingsReset = { cancellationDepths.append(navigation.settings.count) }

        navigation.selectWalletPresentation(selection)
        #expect(cancellationDepths.isEmpty)
        navigation.selectWalletPresentation(.local)

        #expect(cancellationDepths == [1])
        #expect(navigation.settings.isEmpty)
    }

    @Test
    func restoresCollectiveSelectionAfterRestart() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let selection = collective("team")
        let first = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)

        first.selectWalletPresentation(selection)

        let restarted = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        #expect(restarted.walletPresentation == selection)
        #expect(restarted.home.isEmpty)
        #expect(restarted.wallet.isEmpty)
        #expect(restarted.settings.isEmpty)
    }

    @Test
    func localSelectionClearsOnlyItsOriginNavigationKey() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let first = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        first.selectWalletPresentation(collective("first-team"))
        let firstKey = try #require(preferences.values.keys.first)
        let otherOrigin = "https://other-portal.example"
        let secondSelection = collective("second-team")
        let second = NavigationStateManager(defaults: preferences.defaults, portalOrigin: otherOrigin)
        #expect(second.walletPresentation == .local)
        second.selectWalletPresentation(secondSelection)
        preferences.defaults.set("synthetic-retained-session", forKey: "retained-platform-state")

        first.selectWalletPresentation(.local)

        #expect(preferences.defaults.object(forKey: firstKey) == nil)
        #expect(preferences.defaults.string(forKey: "retained-platform-state") == "synthetic-retained-session")
        #expect(preferences.values.count == 2)
        #expect(NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin).walletPresentation == .local)
        #expect(NavigationStateManager(defaults: preferences.defaults, portalOrigin: otherOrigin).walletPresentation == secondSelection)
    }

    @Test
    func retainedMpcCannotBecomeAStoredRoot() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        navigation.selectWalletPresentation(.mpc(walletId: "retained-mpc"))
        #expect(navigation.walletPresentation == .local)
        #expect(preferences.values.isEmpty)

        navigation.selectWalletPresentation(collective("team"))
        let key = try #require(preferences.values.keys.first)
        navigation.walletPresentation = .mpc(walletId: "retained-mpc")
        #expect(preferences.values.isEmpty)
        navigation.selectWalletPresentation(collective("team"))
        let retainedSelection = try JSONEncoder().encode(WalletPresentation.mpc(walletId: "retained-mpc"))
        preferences.defaults.set(retainedSelection, forKey: key)
        preferences.defaults.set("synthetic-retained-material", forKey: "retained-mpc-state")

        let restarted = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)

        #expect(restarted.walletPresentation == .local)
        #expect(preferences.defaults.object(forKey: key) == nil)
        #expect(preferences.defaults.string(forKey: "retained-mpc-state") == "synthetic-retained-material")
    }

    @Test(arguments: ["invalid-json", "unknown-case", "invalid-signer", "empty-wallet"])
    func corruptPublicMetadataFallsBackWithoutRemovingOtherPreferences(fixture: String) throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        navigation.selectWalletPresentation(collective("team"))
        let key = try #require(preferences.values.keys.first)
        let data = switch fixture {
        case "invalid-json": Data("not-json".utf8)
        case "unknown-case": Data(#"{"futureWallet":{"walletId":"team"}}"#.utf8)
        case "invalid-signer": Data(#"{"collective":{"signerWalletId":"invalid-wallet-id","walletId":"team"}}"#.utf8)
        default: try JSONEncoder().encode(collective(""))
        }
        preferences.defaults.set(data, forKey: key)
        preferences.defaults.set("synthetic-cold-purpose", forKey: "retained-device-purpose")

        let restarted = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)

        #expect(restarted.walletPresentation == .local)
        #expect(preferences.defaults.object(forKey: key) == nil)
        #expect(preferences.defaults.string(forKey: "retained-device-purpose") == "synthetic-cold-purpose")
    }

    @Test
    func reselectingACollectiveWalletPreservesItsNavigationContext() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        let selection = collective("team")
        navigation.selectWalletPresentation(selection)
        navigation.home.append("collective-home-detail")
        navigation.wallet.append("collective-wallet-detail")
        navigation.settings.append("settings-detail")
        navigation.pendingWalletPath = ["pending-detail"]
        navigation.selectedTab = .wallet

        navigation.selectWalletPresentation(selection)

        #expect(navigation.walletPresentation == selection)
        #expect(navigation.home.count == 1)
        #expect(navigation.wallet.count == 1)
        #expect(navigation.settings.count == 1)
        #expect(navigation.pendingWalletPath.count == 1)
        #expect(navigation.selectedTab == .wallet)
        #expect(NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin).walletPresentation == selection)
    }

    @Test
    func switchingWalletPresentationRemovesAllPreviousNavigationPaths() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        navigation.selectWalletPresentation(collective("previous-team"))
        navigation.home.append("home-detail")
        navigation.wallet.append("wallet-detail")
        navigation.swap.append("legacy-swap-detail")
        navigation.settings.append("settings-detail")
        navigation.pendingWalletPath = ["pending-detail"]
        navigation.detailSwapDestination = PortalSwapDestination(entryAsset: Chain.bitcoinSv.asset.id, id: UUID())
        navigation.detailSwapDepth = 1
        navigation.selectedTab = .wallet

        navigation.selectWalletPresentation(collective("team"))

        #expect(navigation.home.isEmpty)
        #expect(navigation.wallet.isEmpty)
        #expect(navigation.swap.isEmpty)
        #expect(navigation.settings.isEmpty)
        #expect(navigation.pendingWalletPath.isEmpty)
        #expect(navigation.detailSwapDestination == nil)
        #expect(navigation.detailSwapDepth == 0)
        #expect(navigation.selectedTab == .wallet)
    }

    @Test
    func openingAPersonalAssetClearsThePersistedCollectiveChoice() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        navigation.selectWalletPresentation(collective("team"))
        navigation.wallet.append("collective-detail")

        navigation.openAsset(Chain.bitcoinSv.asset)

        #expect(navigation.walletPresentation == .local)
        #expect(navigation.wallet.count == 1)
        #expect(preferences.values.isEmpty)
        #expect(NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin).walletPresentation == .local)
    }

    @Test(arguments: PreservedOwner.allCases)
    func preservesOnlyTheExactPassedNavigationOwner(owner: PreservedOwner) throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        navigation.selectWalletPresentation(collective("previous-team"))
        navigation.home.append("home-detail")
        navigation.swap.append("swap-detail")
        navigation.wallet.append("wallet-detail")
        navigation.settings.append("settings-detail")
        navigation.pendingWalletPath = ["pending-detail"]
        navigation.detailSwapDestination = PortalSwapDestination(entryAsset: Chain.bitcoinSv.asset.id, id: UUID())
        navigation.detailSwapDepth = 2
        navigation.selectedTab = .settings
        let preserved = path(owner, navigation: navigation)
        let expected = preserved.path
        var settingsResetDepths: [Int] = []
        var removed = 0
        let observer = preserved.observeRemoval { removed += 1 }
        defer { preserved.removeRemovalObserver(observer) }
        navigation.onSettingsReset = { settingsResetDepths.append(navigation.settings.count) }
        defer { navigation.onSettingsReset = nil }

        navigation.selectWalletPresentation(.local, preservingNavigation: preserved)

        #expect(path(owner, navigation: navigation) === preserved)
        #expect(preserved.path == expected)
        #expect(removed == 0)
        for other in PreservedOwner.allCases where other != owner {
            #expect(path(other, navigation: navigation).isEmpty)
        }
        #expect(settingsResetDepths == (owner == .settings ? [] : [1]))
        #expect(navigation.detailSwapDestination == nil)
        #expect(navigation.detailSwapDepth == 0)
        #expect(navigation.pendingWalletPath.isEmpty)
        #expect(navigation.selectedTab == .settings)
        #expect(navigation.walletPresentation == .local)
        #expect(NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin).walletPresentation == .local)
    }

    @Test
    func aForeignNavigationObjectWithIdenticalContentsDoesNotPreserveSettings() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        navigation.selectWalletPresentation(collective("previous-team"))
        navigation.settings.append("same-detail")
        let foreign = NavigationPathState()
        foreign.append("same-detail")
        let foreignPath = foreign.path
        #expect(foreign.path == navigation.settings.path)
        var cancellationDepths: [Int] = []
        navigation.onSettingsReset = { cancellationDepths.append(navigation.settings.count) }
        defer { navigation.onSettingsReset = nil }

        navigation.selectWalletPresentation(.local, preservingNavigation: foreign)

        #expect(cancellationDepths == [1])
        #expect(navigation.settings.isEmpty)
        #expect(foreign.path == foreignPath)
    }

    @Test
    func aLaterManualSelectionStillRunsTheSettingsHookAndRemovesThePreservedPath() throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        navigation.selectWalletPresentation(collective("previous-team"))
        navigation.settings.append("cold-entry")
        var cancellationDepths: [Int] = []
        navigation.onSettingsReset = { cancellationDepths.append(navigation.settings.count) }
        defer { navigation.onSettingsReset = nil }
        navigation.selectWalletPresentation(.local, preservingNavigation: navigation.settings)
        #expect(cancellationDepths.isEmpty)
        #expect(navigation.settings.count == 1)
        navigation.settings.append("cold-review")

        navigation.selectWalletPresentation(collective("next-team"))

        #expect(cancellationDepths == [2])
        #expect(navigation.settings.isEmpty)
    }

    @Test(.timeLimit(.minutes(1)))
    func theNativeSettingsStackRetainsItsEntryAndReviewUntilActualBackNavigation() async throws {
        let preferences = try Preferences()
        defer { preferences.clear() }
        let navigation = NavigationStateManager(defaults: preferences.defaults, portalOrigin: origin)
        navigation.selectWalletPresentation(collective("previous-team"))
        navigation.selectedTab = .settings
        let state = PreservationHostState(navigation: navigation)
        navigation.onSettingsReset = { state.settingsResets += 1 }
        let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first { $0.activationState == .foregroundActive })
        let previousKeyWindow = scene.windows.first { $0.isKeyWindow }
        let animations = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
        let window = UIWindow(windowScene: scene)
        window.frame = scene.coordinateSpace.bounds
        window.rootViewController = UIHostingController(rootView: PreservationHost(state: state))
        window.makeKeyAndVisible()
        defer {
            navigation.onSettingsReset = nil
            window.isHidden = true
            window.rootViewController = nil
            previousKeyWindow?.makeKey()
            UIView.setAnimationsEnabled(animations)
        }
        await state.rootAppeared.wait()
        navigation.settings.append(PreservationRoute.entry)
        await state.entryAppeared.wait()
        let observer = navigation.settings.observeRemoval { state.removalDepths.append(navigation.settings.count) }
        defer { navigation.settings.removeRemovalObserver(observer) }
        navigation.settings.append(PreservationRoute.review)
        await state.reviewAppeared.wait()
        let expected = navigation.settings.path
        navigation.home.append("other-tab-detail")
        navigation.wallet.append("other-wallet-detail")

        navigation.selectWalletPresentation(.local, preservingNavigation: navigation.settings)

        try #require(navigation.settings.path == expected)
        #expect(state.settingsResets == 0)
        #expect(state.removalDepths.isEmpty)
        state.revision += 1
        await state.rebuilt.wait()
        #expect(navigation.settings.path == expected)
        #expect(navigation.home.isEmpty && navigation.wallet.isEmpty)
        navigation.settings.removeLast()
        await state.entryAppeared.wait()
        #expect(navigation.settings.count == 1)
        #expect(state.removalDepths.isEmpty)
        navigation.settings.removeLast()
        await state.rootAppeared.wait()
        #expect(navigation.settings.isEmpty)
        #expect(state.removalDepths == [1])
        #expect(state.settingsResets == 0)
        navigation.selectWalletPresentation(collective("next-team"))
        #expect(state.settingsResets == 1)
    }

    enum PreservedOwner: CaseIterable, Equatable, Sendable { case home, swap, wallet, settings }

    private func path(_ owner: PreservedOwner, navigation: NavigationStateManager) -> NavigationPathState {
        switch owner {
        case .home: navigation.home
        case .swap: navigation.swap
        case .wallet: navigation.wallet
        case .settings: navigation.settings
        }
    }

    enum PreservationRoute: String, Hashable, Codable { case entry, review }

    @Observable
    @MainActor
    final class PreservationHostState {
        let navigation: NavigationStateManager
        var revision = 0
        var settingsResets = 0
        var removalDepths: [Int] = []
        let rootAppeared = PreservationSignal()
        let entryAppeared = PreservationSignal()
        let reviewAppeared = PreservationSignal()
        let rebuilt = PreservationSignal()

        init(navigation: NavigationStateManager) {
            self.navigation = navigation
        }
    }

    private struct PreservationHost: View {
        @Bindable var state: PreservationHostState

        var body: some View {
            NavigationStack(path: state.navigation.settings.binding) {
                Text(state.navigation.walletPresentation.signerWalletId == nil ? "Personal profile" : "Collective profile")
                    .onAppear { state.rootAppeared.send() }
                    .navigationDestination(for: PreservationRoute.self) { route in
                        switch route {
                        case .entry:
                            Text("Cold entry").onAppear { state.entryAppeared.send() }
                        case .review:
                            Text("Cold review").onAppear { state.reviewAppeared.send() }
                        }
                    }
            }
            .environment(state.navigation.settings)
            .environment(\.scenePhase, .active)
            .environment(\.locale, Locale(identifier: state.revision.isMultiple(of: 2) ? "en_US" : "zh_Hans"))
            .onChange(of: state.revision) { _, _ in state.rebuilt.send() }
        }
    }

    @MainActor
    final class PreservationSignal {
        private let events = AsyncStream<Void>.makeStream()

        func send() {
            events.continuation.yield()
        }

        func wait() async {
            var iterator = events.stream.makeAsyncIterator()
            _ = await iterator.next()
        }
    }

    private var origin: String { "https://paymail.bsvhome.com" }

    private func collective(_ walletId: String) -> WalletPresentation {
        .collective(signerWalletId: .multicoin(address: "public-signer"), walletId: walletId)
    }
}

private struct Preferences {
    let suiteName: String
    let defaults: UserDefaults

    init() throws {
        suiteName = "WalletPresentationTests." + UUID().uuidString
        defaults = try #require(UserDefaults(suiteName: suiteName))
    }

    var values: [String: Any] {
        defaults.persistentDomain(forName: suiteName) ?? [:]
    }

    func clear() {
        defaults.removePersistentDomain(forName: suiteName)
    }
}
