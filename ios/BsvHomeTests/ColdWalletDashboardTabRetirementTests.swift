import BsvColdWallet
import Foundation
@testable import BsvHome
import BsvHomeCore
import BsvHomeCorePrimitivesTestKit
import Testing

@MainActor
struct ColdWalletDashboardTabRetirementTests {
    @Test
    func realTabAssignmentInvalidatesTheDashboardModelBeforeTheNewTabCommits() {
        let navigation = NavigationStateManager(defaults: .standard, portalOrigin: "")
        let fixture = context(in: navigation)
        fixture.context.observeTabExit(in: navigation, tab: .home)

        navigation.selectedTab = .wallet

        #expect(fixture.port.observedTabs == [.home])
        #expect(fixture.context.model.isCancelled)
        #expect(navigation.selectedTab == .wallet)
    }

    @Test
    func internalPushesAndSameTabAssignmentsPreserveDashboardAndAnUnrelatedRealFinancialScope() async throws {
        let navigation = NavigationStateManager(defaults: .standard, portalOrigin: "")
        let fixture = context(in: navigation)
        fixture.context.observeTabExit(in: navigation, tab: .home)
        let foreign = try await BsvMultisigActionScopeFixture.make()
        let scope = try foreign.beginRetryFunding()
        navigation.home.append("dashboard-internal-pairing")
        navigation.home.append("dashboard-internal-review")
        navigation.home.removeLast()
        navigation.selectedTab = .home
        #expect(fixture.port.observedTabs.isEmpty)
        #expect(!fixture.context.model.isCancelled)
        try scope.check()

        navigation.selectedTab = .wallet

        #expect(fixture.port.observedTabs == [.home])
        #expect(fixture.context.model.isCancelled)
        try scope.check()
        #expect(foreign.networkRequestCount == 0)
        #expect(foreign.credentialAccessCount == 0)
    }

    @Test
    func retiringAnOldDashboardRemovesOnlyItsTokenAndCannotCancelAnotherTabsReplacement() {
        let navigation = NavigationStateManager(defaults: .standard, portalOrigin: "")
        let previous = context(in: navigation)
        let replacement = context(in: navigation)
        previous.context.observeTabExit(in: navigation, tab: .home)
        replacement.context.observeTabExit(in: navigation, tab: .home)
        previous.context.cancel()
        #expect(previous.port.observedTabs == [.home])
        #expect(replacement.port.observedTabs.isEmpty)

        navigation.selectedTab = .wallet

        #expect(previous.port.observedTabs == [.home])
        #expect(replacement.port.observedTabs == [.home])
        let wallet = context(in: navigation)
        wallet.context.observeTabExit(in: navigation, tab: .wallet)
        previous.context.cancel()
        replacement.context.cancel()
        #expect(wallet.port.observedTabs.isEmpty)
        #expect(!wallet.context.model.isCancelled)

        navigation.selectedTab = .settings

        #expect(wallet.port.observedTabs == [.wallet])
        #expect(wallet.context.model.isCancelled)
    }

    private func context(in navigation: NavigationStateManager) -> (context: BsvColdWalletContext, port: CancellationPort) {
        let port = CancellationPort(noHandle: .init())
        port.setTabProbe { MainActor.assumeIsolated { navigation.selectedTab } }
        let model = BsvColdWalletViewModel(service: port, mode: .unified)
        return (BsvColdWalletContext(model: model, authorization: BsvColdWalletPasswordAuthorization(store: BsvHomeSecureStoreMock(), isForeground: { true })), port)
    }

    private final class CancellationPort: BsvHomeBsvColdWalletService, @unchecked Sendable {
        private let lock = NSLock()
        private var tabs: [TabItem] = []
        private var tabProbe: @Sendable () -> TabItem = { .home }
        var observedTabs: [TabItem] { lock.withLock { tabs } }

        func setTabProbe(_ value: @escaping @Sendable () -> TabItem) {
            lock.withLock { tabProbe = value }
        }

        override func invalidateActions() throws {
            let probe = lock.withLock { tabProbe }
            let tab = probe()
            lock.withLock { tabs.append(tab) }
        }
    }
}
