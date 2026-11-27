// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Contacts
import BsvHomeCoreServices
import InAppNotifications
import Localization
import MarketInsight
import PriceAlerts
import Primitives
import PrimitivesComponents
import Settings
import Store
import Support
import SwiftUI
import Transfer
import WalletConnector

struct SettingsNavigationView: View {
    @Environment(NavigationPathState.self) private var navigation
    @Environment(\.navigationState) private var navigationState
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.navigationHandler) private var navigationHandler
    @Environment(\.walletConnector) private var walletConnector
    @Environment(\.observablePreferences) private var observablePreferences
    @Environment(\.walletConnectorPresenter) private var walletConnectorPresenter
    @Environment(\.viewModelFactory) private var viewModelFactory
    @Environment(\.navigationPresenter) private var presenter

    private let onCreateWallet: () -> Void
    private let onImportWallet: () -> Void
    private let onActivity: () -> Void
    private let onPaymail: () -> Void
    private let onAddressManagement: () -> Void
    private let onMultisig: (() -> Void)?
    private let onAccount: (() -> Void)?
    private let selectedWalletName: String?
    @State private var transferPasswordModel: TransferPasswordViewModel?
    @State private var transferPasswordError: ToastMessage?
    @State private var passwordRemovalObserver: UUID?
    @State private var passwordTabExitObserver: UUID?
    let walletId: WalletId
    @Binding var isPresentingSupport: Bool

    init(
        walletId: WalletId,
        isPresentingSupport: Binding<Bool>,
        onCreateWallet: @escaping () -> Void,
        onImportWallet: @escaping () -> Void,
        onActivity: @escaping () -> Void,
        onPaymail: @escaping () -> Void,
        onAddressManagement: @escaping () -> Void,
        onMultisig: (() -> Void)? = nil,
        onAccount: (() -> Void)? = nil,
        selectedWalletName: String? = nil,
    ) {
        self.walletId = walletId
        self.onCreateWallet = onCreateWallet
        self.onImportWallet = onImportWallet
        self.onActivity = onActivity
        self.onPaymail = onPaymail
        self.onAddressManagement = onAddressManagement
        self.onMultisig = onMultisig
        self.onAccount = onAccount
        self.selectedWalletName = selectedWalletName
        _isPresentingSupport = isPresentingSupport
    }

    var body: some View {
        SettingsScene(
            model: viewModelFactory.settingsScene(),
            isPresentingWallets: presenter.isPresentingWallets,
            onCreateWallet: onCreateWallet,
            onImportWallet: onImportWallet,
            onActivity: onActivity,
            onPaymail: onPaymail,
            onAddressManagement: onAddressManagement,
            onMultisig: onMultisig,
            onAccount: onAccount,
            selectedWalletName: selectedWalletName,
        )
        .wowPayTabStatusBarBackground()
        .navigationBarTitleDisplayMode(.inline)
        .navigationDestination(for: Scenes.Security.self) { _ in
            SecurityScene(model: viewModelFactory.securityScene(onManagePassword: presentPasswordManagement))
                .toolbar(.visible, for: .navigationBar)
        }
        .sheet(item: Binding(
            get: { transferPasswordModel },
            set: { [presentedId = transferPasswordModel?.id] replacement in
                guard transferPasswordModel?.id == presentedId else { return }
                if transferPasswordModel?.id != replacement?.id {
                    transferPasswordModel?.cancel()
                    removePasswordObservers()
                }
                transferPasswordModel = replacement
            },
        )) { password in
            NavigationStack { TransferPasswordScene(model: password) }
                .presentationDetents([.medium, .large])
        }
        .toast(message: $transferPasswordError)
        .onChange(of: walletId) { _, _ in cancelPasswordManagement() }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                cancelPasswordManagement()
            }
        }
        .navigationDestination(for: Scenes.Notifications.self) { _ in
            NotificationsScene(
                model: viewModelFactory.notificationsScene(),
            )
            .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.PriceAlerts.self) { _ in
            PriceAlertsNavigationView(
                model: viewModelFactory.priceAlertsScene(),
            )
            .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.AssetPriceAlert.self) {
            AssetPriceAlertsScene(
                model: viewModelFactory.assetPriceAlertsScene(walletId: walletId, asset: $0.asset),
            )
            .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.Price.self) { scene in
            ChartScene(
                model: viewModelFactory.chartScene(
                    asset: scene.asset,
                    walletId: walletId,
                    onSetPriceAlert: { presenter.isPresentingPriceAlert.wrappedValue = $0 },
                ),
            )
            .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.Chains.self) { _ in
            ChainListSettingsScene(model: viewModelFactory.chainListSettingsScene())
                .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.ServiceStatus.self) { _ in
            ServiceStatusScene(model: viewModelFactory.serviceStatusScene())
                .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.AboutUs.self) { _ in
            AboutUsScene(
                model: viewModelFactory.aboutUsScene(),
            )
            .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.WalletConnect.self) { _ in
            ConnectionsScene(
                model: ConnectionsViewModel(
                    connector: walletConnector,
                    walletConnectorPresenter: walletConnectorPresenter,
                ),
            )
            .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.Developer.self) { _ in
            if observablePreferences.isDeveloperEnabled {
                DeveloperScene(model: viewModelFactory.developerScene(walletId: walletId))
                    .toolbar(.visible, for: .navigationBar)
            }
        }
        .navigationDestination(for: Scenes.DeveloperPayments.self) { _ in
            DeveloperPaymentsScene { payload in
                Task { await navigationHandler.handle(code: payload) }
            }
            .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.InAppNotifications.self) { _ in
            if let model = viewModelFactory.inAppNotificationsScene() {
                InAppNotificationsScene(model: model)
                    .toolbar(.visible, for: .navigationBar)
            }
        }
        .navigationDestination(for: Scenes.Preferences.self) { _ in
            PreferencesScene(model: viewModelFactory.preferencesScene())
                .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.Currency.self) { _ in
            CurrencyScene(model: viewModelFactory.currencyScene())
                .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.Appearance.self) { _ in
            AppearanceScene(model: AppearanceViewModel(preferences: observablePreferences))
                .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.Referral.self) { scene in
            if let model = viewModelFactory.rewardsScene(activateCode: scene.code) {
                RewardsScene(model: model)
                    .toolbar(.visible, for: .navigationBar)
            }
        }
        .navigationDestination(for: Scenes.ChainSettings.self) {
            ChainSettingsScene(model: viewModelFactory.chainSettingsScene(chain: $0.chain))
                .toolbar(.visible, for: .navigationBar)
        }
        .navigationDestination(for: Scenes.Contacts.self) { _ in
            ContactsNavigationView(model: viewModelFactory.contactsScene())
                .toolbar(.visible, for: .navigationBar)
        }
        .sheet(isPresented: $isPresentingSupport) {
            NavigationStack {
                SupportChatScene(model: viewModelFactory.supportChatScene())
                    .toolbarDismissItem(type: .close, placement: .topBarLeading)
            }
            .environment(\.openURL, OpenURLAction { url in
                guard navigationHandler.open(url: url) else { return .systemAction }
                isPresentingSupport = false
                return .handled
            })
        }
    }

    private func presentPasswordManagement(_ action: SecurityViewModel.PasswordManagement) {
        var securityPath = navigation.path
        guard !securityPath.isEmpty else { return }
        securityPath.removeLast()
        securityPath.append(Scenes.Security())
        guard securityPath == navigation.path, navigation === navigationState.settings,
              navigationState.selectedTab == .settings, scenePhase == .active
        else { return }
        cancelPasswordManagement()
        do {
            let password = switch action {
            case .online: try viewModelFactory.onlineTransferPasswordScene(walletId: walletId)
            case .offline: try viewModelFactory.offlineSigningPasswordScene(walletId: walletId)
            }
            transferPasswordModel = password
            let id = password.id
            passwordRemovalObserver = navigation.observeRemoval {
                guard transferPasswordModel?.id == id else { return }
                cancelPasswordManagement()
            }
            passwordTabExitObserver = navigationState.observeTabExit(tab: .settings) {
                guard transferPasswordModel?.id == id else { return }
                cancelPasswordManagement()
            }
        } catch {
            transferPasswordError = ToastMessage(title: action == .online ? Localized.wowpayTransferPasswordUnavailable : Localized.wowpayOfflineSigningPasswordUnavailable, image: "exclamationmark.triangle")
        }
    }

    private func cancelPasswordManagement() {
        transferPasswordModel?.cancel()
        transferPasswordModel = nil
        removePasswordObservers()
    }

    private func removePasswordObservers() {
        if let passwordRemovalObserver {
            navigation.removeRemovalObserver(passwordRemovalObserver)
        }
        passwordRemovalObserver = nil
        if let passwordTabExitObserver {
            navigationState.removeTabExitObserver(passwordTabExitObserver)
        }
        passwordTabExitObserver = nil
    }
}
