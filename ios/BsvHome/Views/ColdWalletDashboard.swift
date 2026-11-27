import BsvColdWallet
import Components
import Localization
import Primitives
import Style
import SwiftUI
import WalletTab

struct ColdWalletDashboard: View {
    @Environment(\.viewModelFactory) private var factory
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.navigationState) private var navigationState
    @State private var context: BsvColdWalletContext?
    @State private var failed = false
    @State private var unsupportedEntry: (id: UUID, path: NavigationPath)?

    let wallet: Wallet
    let navigation: NavigationPathState
    let tab: TabItem
    private var isActive: Bool { navigationState.selectedTab == tab }
    let header: (@escaping () -> Void) -> AnyView

    private enum Route: Hashable, Codable {
        case unsupported(UUID)
    }

    var body: some View {
        Group {
            if let context {
                BsvColdWalletScene(model: context.model, navigation: navigation, dashboard: dashboard, onRequestSigning: coldWalletRequest(for: context, entry: .scanRequest), onRequestPairing: coldWalletRequest(for: context, entry: .pairing))
                    .id(ObjectIdentifier(context))
                    .bsvColdWalletPassword(authorization: context.authorization)
            } else {
                ScrollView {
                    VStack(spacing: Spacing.medium) {
                        header(openUnsupported)
                        if failed {
                            ContentUnavailableView {
                                Label(Localized.coldWalletTitle, systemImage: "exclamationmark.triangle")
                            } description: {
                                Text(Localized.coldWalletStartupStorageError)
                            } actions: {
                                Button(Localized.Common.tryAgain, action: load)
                            }
                        } else {
                            LoadingView()
                        }
                    }
                    .padding(.horizontal, Spacing.wowPay.page)
                }
                .wowPayBackground(.solid)
            }
        }
        .navigationDestination(for: Route.self) { route in
            switch route {
            case let .unsupported(id): unsupported(id: id)
            }
        }
        .task(id: isActive && scenePhase == .active) {
            guard isActive, scenePhase == .active, !Task.isCancelled else { return }
            if context == nil {
                load()
            }
        }
        .onChange(of: isActive) { _, active in
            if !active {
                retire()
            }
        }
        .onChange(of: context?.model.isCancelled) { _, cancelled in
            if cancelled == true {
                retire()
                if isActive, scenePhase == .active {
                    load()
                }
            }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                retire()
            }
        }
    }

    private func dashboard(balance: AnyView, operations: AnyView, status: AnyView?) -> AnyView {
        if tab == .home {
            AnyView(WowPayHomeContent(
                header: header(openUnsupported),
                balance: balance,
                isReceiveEnabled: true,
                isSendEnabled: true,
                onSelectScan: openUnsupported,
                onSelectReceive: openUnsupported,
                onSelectSend: openUnsupported,
                operations: operations,
                status: status,
            ))
        } else {
            AnyView(ScrollView {
                VStack(spacing: Spacing.medium) {
                    header(openUnsupported)
                    balance
                    operations
                    if let status {
                        status
                    }
                }
                .padding(.horizontal, Spacing.wowPay.page)
                .padding(.bottom, Spacing.wowPay.coldWalletPageBottom)
            }
            .wowPayBackground(.solid))
        }
    }

    private func unsupported(id: UUID) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.medium) {
                VStack(alignment: .leading, spacing: Spacing.space8) {
                    Text(Localized.coldWalletHomeUnsupported).font(.wowPay.coldWalletUnsupportedTitle)
                        .foregroundStyle(Colors.WowPay.heading)
                    Text(Localized.coldWalletUseObserver).font(.wowPay.coldWalletDescription)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                Text(Localized.coldWalletSignerRole).font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .frame(maxWidth: .infinity, alignment: .leading).wowPayCard()
                Button(Localized.coldWalletOpenAssetsToSign) { openAssets(from: id) }
                    .buttonStyle(.wowPay())
                    .accessibilityIdentifier("coldWalletOpenAssetsToSign")
            }
            .padding(.horizontal, Spacing.wowPay.page)
            .padding(.bottom, Spacing.wowPay.coldWalletPageBottom)
        }
        .wowPayBackground(.solid)
        .navigationTitle("")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .accessibilityIdentifier("coldWalletUnsupportedPage")
    }

    private func coldWalletRequest(for expected: BsvColdWalletContext, entry: Scenes.BsvColdWallet.Entry) -> @MainActor (String) -> Void {
        let path = navigation.path
        return { walletId in
            guard context === expected, !expected.model.isCancelled, isActive, scenePhase == .active, navigation.path == path else { return }
            navigation.append(Scenes.BsvColdWallet(walletId: walletId, entry: entry))
        }
    }

    private func openUnsupported() {
        guard isActive, scenePhase == .active else { return }
        let id = UUID()
        navigation.append(Route.unsupported(id))
        unsupportedEntry = (id, navigation.path)
    }

    private func openAssets(from id: UUID) {
        guard let entry = unsupportedEntry, entry.id == id, navigation.path == entry.path else { return }
        unsupportedEntry = nil
        navigation.removeLast()
        navigationState.select(tab: .wallet)
    }

    private func load() {
        guard isActive, !Task.isCancelled else { return }
        do {
            let created = try factory.bsvColdWalletContext(walletId: wallet.id.id, onComplete: navigation.reset)
            created.observeTabExit(in: navigationState, tab: tab)
            context = created
            failed = false
        } catch {
            failed = true
        }
    }

    private func retire() {
        context?.cancel()
        context = nil
    }
}
