// Copyright (c). Gem Wallet. All rights reserved.

import Assets
import BsvColdWallet
import BsvMultisig
import Components
import enum BsvHomeCore.BsvHomeBsvColdWalletRole
import protocol BsvHomeCore.BsvHomePortalSwapServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import Onboarding
import PriceAlerts
import Primitives
import QRScanner
import Style
import SwiftUI
import Transactions
import WalletTab

struct MainTabView: View {
    @Environment(\.navigationState) private var navigationState
    @Environment(\.navigationHandler) private var navigationHandler
    @Environment(\.navigationPresenter) private var presenter
    @Environment(\.viewModelFactory) private var viewModelFactory
    @Environment(\.scenePhase) private var scenePhase
    @Environment(RootSceneViewModel.self) private var rootModel

    let wallet: Wallet
    let directory: CollectiveWalletDirectory

    @State private var model: MainTabViewModel
    @State private var sheet: Sheet?
    @State private var portalPaymentPresenter = PortalSwapPaymentPresenter()
    @State private var importWelcomeDepth = 0
    @State private var createIntroDepth = 0
    @State private var createWalletFlow: CreateWalletFlow?
    @State private var importedWallet: Wallet?
    @State private var selectedBalanceAsset: WowPayAsset = .bsv
    @State private var collectiveOnboarding: CollectiveOnboardingModel?
    @State private var collectiveOverview: CollectiveOnboardingModel?
    @State private var collectiveProfileDepth = 0
    @State private var identitySelectionTask: Task<Void, Never>?
    @State private var isSelectingIdentity = false
    @State private var identitySelectionId: UUID?
    @State private var inlineImportModel: ImportWalletViewModel?
    @State private var inlineImportDepth = 0
    @State private var walletSelectorPresentation: WalletPresentation?
    @State private var walletSelectorEntry: WalletSelectorEntry?
    @State private var publicFunding: BsvPublicFundingContext?
    @State private var multisigScanner = BsvMultisigScannerPresentation()
    @State private var tabExitOwnerId: UUID?
    @State private var coldWalletRoleState = ColdWalletRoleState.loading

    private enum ColdWalletRoleState {
        case loading
        case ready(Wallet, BsvHomeBsvColdWalletRole)
        case failed
    }

    private struct CollectiveProfileDestination: Hashable, Codable {
        let id: UUID
    }

    private struct WalletSelectorDestination: Hashable, Codable {
        let id: UUID
    }

    private struct WalletSelectorEntry {
        let destination: WalletSelectorDestination
        let navigation: NavigationPathState
        let parentPath: NavigationPath
        let removalObserver: UUID
    }

    private enum Sheet: String, Identifiable {
        case receive
        var id: String { rawValue }
    }

    private enum ProfileDestination {
        case activity, paymail, receive
    }

    private var isSettingsTabBarVisible: Bool {
        let depth = navigationState.settings.count
        return (importWelcomeDepth == 0 || depth < importWelcomeDepth)
            && (createIntroDepth == 0 || depth < createIntroDepth)
    }

    private var multisigModel: BsvMultisigViewModel? {
        switch navigationState.walletPresentation {
        case let .collective(signerWalletId, _), let .collectiveOverview(signerWalletId):
            directory.model(for: signerWalletId)
        case .local, .mpc:
            directory.model(for: wallet.id)
        }
    }

    private var tabViewSelection: Binding<TabItem> {
        Binding(
            get: { navigationState.selectedTab },
            set: { onSelect(tab: $0) },
        )
    }

    private var portalPaymentBinding: Binding<PortalSwapPaymentContext?> {
        Binding(
            get: { portalPaymentPresenter.payment },
            set: { payment in
                if payment == nil {
                    portalPaymentPresenter.cancel()
                }
            },
        )
    }

    init(wallet: Wallet, directory: CollectiveWalletDirectory) {
        self.wallet = wallet
        self.directory = directory
        _model = State(initialValue: MainTabViewModel(wallet: wallet))
    }

    var body: some View {
        TabView(selection: tabViewSelection) {
            NavigationStack(path: navigationState.home.binding) {
                walletRoot(presentation: .home, path: navigationState.home.binding)
                    .navigationDestination(for: WalletSelectorDestination.self) { _ in
                        walletSelector(navigation: navigationState.home)
                    }
                    .navigationDestination(for: BsvPublicFundingDestination.self, destination: publicFundingContent)
                    .navigationDestination(for: CollectiveScannedSendDestination.self) { collectiveScannedSend($0, navigation: navigationState.home) }
                    .environment(navigationState.home)
                    .id(wallet.id)
                    .bsvColdWalletNavigation(model: rootModel.coldWalletContext(in: navigationState.home)?.model, navigation: navigationState.home, onScanRecipient: rootModel.coldWalletContext(in: navigationState.home)?.onScanRecipient, onInitialPageBack: rootModel.coldWalletContext(in: navigationState.home)?.onInitialPageBack)
                    .navigationDestination(for: Scenes.BsvColdWallet.self) { ColdWalletDestination(scene: $0, navigation: navigationState.home) }
                    .safeAreaInset(edge: .bottom, spacing: .zero) {
                        if !presenter.isPresentingWallets.wrappedValue {
                            tabBar
                        }
                    }
            }
            .toolbar(.hidden, for: .tabBar)
            .tabItem { tabItem(Localized.wowpayHome, Image(systemName: "house.fill")) }
            .tag(TabItem.home)
            NavigationStack(path: navigationState.wallet.binding) {
                walletRoot(presentation: .assets, path: navigationState.wallet.binding)
                    .navigationDestination(for: WalletSelectorDestination.self) { _ in
                        walletSelector(navigation: navigationState.wallet)
                    }
                    .navigationDestination(for: BsvPublicFundingDestination.self, destination: publicFundingContent)
                    .navigationDestination(for: CollectiveScannedSendDestination.self) { collectiveScannedSend($0, navigation: navigationState.wallet) }
                    .environment(navigationState.wallet)
                    .id(wallet.id)
                    .bsvColdWalletNavigation(model: rootModel.coldWalletContext(in: navigationState.wallet)?.model, navigation: navigationState.wallet, onScanRecipient: rootModel.coldWalletContext(in: navigationState.wallet)?.onScanRecipient, onInitialPageBack: rootModel.coldWalletContext(in: navigationState.wallet)?.onInitialPageBack)
                    .navigationDestination(for: Scenes.BsvColdWallet.self) { ColdWalletDestination(scene: $0, navigation: navigationState.wallet) }
                    .safeAreaInset(edge: .bottom, spacing: .zero) {
                        if navigationState.detailSwapDestination == nil, !presenter.isPresentingWallets.wrappedValue {
                            tabBar
                        }
                    }
            }
            .toolbar(.hidden, for: .tabBar)
            .tabItem {
                tabItem(Localized.Assets.title, Images.Tabs.wallet)
            }
            .tag(TabItem.wallet)

            NavigationStack(path: settingsPathBinding) {
                SettingsNavigationView(
                    walletId: wallet.id,
                    isPresentingSupport: presenter.isPresentingSupport,
                    onCreateWallet: openCreateWalletIntro,
                    onImportWallet: {
                        if navigationState.settings.append(WalletTabDestination.importWelcome) {
                            importWelcomeDepth = navigationState.settings.count
                        }
                    },
                    onActivity: { openProfile(.activity) },
                    onPaymail: { openProfile(.paymail) },
                    onAddressManagement: { openProfile(.receive) },
                    selectedWalletName: selectedWalletName,
                )
                .navigationDestination(for: WalletSelectorDestination.self) { _ in
                    walletSelector(navigation: navigationState.settings)
                }
                .id(wallet.id)
                .bsvColdWalletNavigation(model: rootModel.coldWalletContext(in: navigationState.settings)?.model, navigation: navigationState.settings, onScanRecipient: rootModel.coldWalletContext(in: navigationState.settings)?.onScanRecipient, onInitialPageBack: rootModel.coldWalletContext(in: navigationState.settings)?.onInitialPageBack)
                .bsvMultisigNavigation(model: multisigModel, navigationPath: settingsPathBinding, identityContextForRoute: collectiveIdentityContext)
                .environment(navigationState.settings)
                .safeAreaInset(edge: .bottom, spacing: .zero) {
                    if isSettingsTabBarVisible, !presenter.isPresentingWallets.wrappedValue {
                        tabBar
                    }
                }
                .navigationDestination(for: WalletTabDestination.self) { destination in
                    switch destination {
                    case .activity:
                        TransactionsNavigationView(model: viewModelFactory.transactionsScene(wallet: wallet, type: .all))
                    case .paymail:
                        if isPersonalWalletActive {
                            WowPayPaymailScene(model: viewModelFactory.paymailScene(walletId: wallet.id))
                        } else {
                            unavailablePersonalAction
                        }
                    case .importWelcome:
                        ImportWalletWelcomeScene(
                            onImportWallet: { navigationState.settings.append(WalletTabDestination.importMethod) },
                            onCreateWallet: openCreateWalletIntro,
                        )
                    case .createIntro:
                        CreateWalletMethodScene(onCreateOnline: startCreateWalletFlow, onCreateCold: { openColdWallet(.create) }, onCreateMultisig: { openCollectiveOnboarding(.create) })
                    case .importMethod:
                        ImportWalletMethodScene(
                            onPhrase: { openImportInput(.phrase) },
                            onPrivateKey: { openImportInput(.privateKey) },
                            onWatch: { openImportInput(.address) },
                            onColdWallet: { openColdWallet(.recover) },
                            onConnectColdWallet: { openColdWallet(.connect) },
                            onJoinMultisig: { openCollectiveOnboarding(.importChoice) },
                        )
                    case .coldWallet:
                        ColdWalletDestination(scene: Scenes.BsvColdWallet(walletId: wallet.id.id), navigation: navigationState.settings)
                    case .importPhrase:
                        importInputScene(kind: .phrase)
                    case .importPrivateKey:
                        importInputScene(kind: .privateKey)
                    case .importWatch:
                        importInputScene(kind: .address)
                    }
                }
                .navigationDestination(for: BsvPublicFundingDestination.self, destination: publicFundingContent)
                .navigationDestination(for: Scenes.BsvColdWallet.self) { ColdWalletDestination(scene: $0, navigation: navigationState.settings) }
                .navigationDestination(for: CollectiveScannedSendDestination.self) { collectiveScannedSend($0, navigation: navigationState.settings) }
                .environment(\.bsvMultisigRecipientScanner, recipientScanner(navigation: navigationState.settings))
                .navigationDestination(for: CollectiveProfileDestination.self) { destination in
                    if let collectiveOnboarding, collectiveOnboarding.id == destination.id {
                        CollectiveOnboardingScene(model: collectiveOnboarding, path: settingsPathBinding)
                    }
                }
                .navigationDestination(for: CreateWalletFlow.Step.self) { step in
                    if let createWalletFlow {
                        CreateWalletFlowScene(flow: createWalletFlow, step: step)
                    } else {
                        EmptyView()
                            .task { navigationState.settings.reset() }
                    }
                }
            }
            .toolbar(.hidden, for: .tabBar)
            .tabItem {
                tabItem(Localized.wowpayProfile, Image(systemName: "person.crop.circle"))
            }
            .badge(model.transactions)
            .tag(TabItem.settings)
        }
        .tint(Colors.WowPay.brand)
        .bsvMultisigPassword(authorization: directory.authorization)
        .sheet(item: multisigScanner.binding) { scan in
            ScanQRCodeNavigationStack(scanType: scan.scanType, action: { code in
                multisigScanner.complete(id: scan.id, value: code)
            })
            .orientationLock(.portrait)
        }
        .onChange(of: presenter.multisigRequest?.id, initial: true) { _, _ in consumeMultisigRequest() }
        .onChange(of: navigationState.walletPresentation) { _, _ in
            publicFunding?.dismiss()
            multisigScanner.cancel()
            presenter.cancelMultisigRequest()
        }
        .onChange(of: navigationState.selectedTab) { _, _ in
            dismissWalletSelector()
            multisigScanner.cancel()
            if let publicFunding, publicFunding.navigation !== activeNavigation {
                publicFunding.dismiss()
            }
        }
        .sheet(item: $sheet) { destination in
            NavigationStack {
                Group {
                    if !isPersonalWalletActive {
                        unavailablePersonalAction
                    } else {
                        switch destination {
                        case .receive:
                            ReceiveNavigationView(
                                model: viewModelFactory.receiveScene(
                                    assetAddress: AssetAddress(asset: Chain.bitcoinSv.asset, address: wallet.accounts.first(where: { $0.chain == .bitcoinSv })?.address ?? ""),
                                    wallet: wallet,
                                ),
                                wallet: wallet,
                                navigationTitle: Localized.wowpayReceivePaymentTitle,
                            )
                        }
                    }
                }
                .toolbarDismissItem(type: .close, placement: .topBarLeading)
            }
        }
        .sheet(item: $importedWallet) { wallet in
            ImportWalletSetupSheet(
                model: viewModelFactory.importWalletScene(onComplete: {}),
                wallet: wallet,
                onComplete: { importedWallet = nil },
            )
        }
        .sheet(item: presenter.isPresentingAssetInput) { input in
            if isPersonalWalletActive {
                SelectedAssetNavigationStack(
                    input: input,
                    wallet: wallet,
                    onComplete: { onComplete(type: input.type) },
                )
            } else {
                unavailablePersonalAction
            }
        }
        .sheet(item: presenter.isPresentingPayment) { input in
            if isPersonalWalletActive {
                switch input {
                case let .confirm(data):
                    ConfirmTransferNavigationStack(wallet: wallet, transferData: data, onComplete: onPaymentComplete)
                case let .recipient(assetInput):
                    SelectedAssetNavigationStack(input: assetInput, wallet: wallet, onComplete: onPaymentComplete)
                case let .selectAsset(type, chains):
                    SelectAssetSceneNavigationStack(
                        model: viewModelFactory.selectAssetScene(wallet: wallet, selectType: type, chains: chains),
                    )
                }
            } else {
                unavailablePersonalAction
            }
        }
        .sheet(item: presenter.isPresentingPriceAlert) { asset in
            if isPersonalWalletActive {
                SetPriceAlertNavigationStack(
                    model: viewModelFactory.setPriceAlertScene(walletId: wallet.id, asset: asset, onComplete: onSetPriceAlertComplete),
                )
            } else {
                unavailablePersonalAction
            }
        }
        .sheet(item: portalPaymentBinding) { payment in
            PortalSwapPaymentNavigationStack(context: payment)
        }
        .onAppear {
            if tabExitOwnerId == nil {
                tabExitOwnerId = navigationState.registerTabExitOwner {
                    if rootModel.appColdWalletNavigation(in: navigationState) != nil {
                        rootModel.activeColdWalletContext?.cancelActions()
                    }
                    directory.cancelActions()
                    collectiveOverview?.featureModel?.cancelActions()
                    collectiveOnboarding?.featureModel?.cancelActions()
                    refreshColdWalletRole()
                }
            }
            portalPaymentPresenter.activate()
            navigationState.onSettingsReset = cancelProfileOnboarding
        }
        .onDisappear {
            if let tabExitOwnerId {
                navigationState.removeTabExitOwner(tabExitOwnerId)
                self.tabExitOwnerId = nil
            }
            portalPaymentPresenter.deactivate()
            publicFunding?.dismiss()
            multisigScanner.cancel()
        }
        .onChange(of: navigationState.selectedTab) { _, _ in cancelInactivePortalPayment() }
        .onChange(of: presenter.isPresentingWallets.wrappedValue) { wasPresenting, isPresenting in
            if isPresenting {
                walletSelectorPresentation = navigationState.walletPresentation
                presentWalletSelector()
            } else {
                dismissWalletSelector()
                let previousPresentation = walletSelectorPresentation
                walletSelectorPresentation = nil
                if wasPresenting, previousPresentation == navigationState.walletPresentation {
                    resumeCollectiveOverviewAfterWalletSelector()
                }
            }
        }
        .onChange(of: navigationState.walletPresentation) { _, _ in
            sheet = nil
            identitySelectionTask?.cancel()
            identitySelectionTask = nil
            isSelectingIdentity = false
            identitySelectionId = nil
            collectiveOverview?.cancel()
            collectiveOverview = nil
            if !isPersonalWalletActive {
                presenter.isPresentingAssetInput.wrappedValue = nil
                presenter.isPresentingPayment.wrappedValue = nil
                presenter.isPresentingPriceAlert.wrappedValue = nil
            }
        }
        .onChange(of: navigationState.settings.count) { _, count in
            if count < importWelcomeDepth {
                importWelcomeDepth = 0
            }
            if createIntroDepth > 0, count <= createIntroDepth {
                createWalletFlow?.discard()
                createWalletFlow = nil
                if count < createIntroDepth {
                    createIntroDepth = 0
                }
            }
        }
        .onChange(of: navigationState.detailSwapDestination) { _, _ in cancelInactivePortalPayment() }
        .onChange(of: navigationState.wallet.count) { _, _ in cancelInactivePortalPayment() }
        .onChange(of: wallet.id) { _, _ in
            publicFunding?.dismiss()
            multisigScanner.cancel()
            presenter.cancelMultisigRequest()
            sheet = nil
            portalPaymentPresenter.cancel()
            createWalletFlow?.discard()
            createWalletFlow = nil
            selectedBalanceAsset = .bsv
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                publicFunding?.dismiss()
                multisigScanner.cancel()
                presenter.cancelMultisigRequest()
                portalPaymentPresenter.cancel()
                createWalletFlow?.discard()
                collectiveOnboarding?.cancel()
                collectiveOverview?.cancel()
                identitySelectionTask?.cancel()
                inlineImportModel?.cancel()
            }
        }
        .toast(message: $model.isPresentingToastMessage)
        .bindQuery(model.transactionsQuery)
        .task(id: wallet) {
            model.transactionsQuery.request.walletId = wallet.id
            refreshColdWalletRole()
        }
        .onChange(of: rootModel.activeColdWalletContext?.id) { previous, current in
            if previous != nil, current == nil {
                refreshColdWalletRole()
                if isCurrentWalletPresentation, coldWalletRole == .signer {
                    navigationState.selectWalletPresentation(.local)
                }
            }
        }
        .connectionStatusBanner()
    }
}

// MARK: - UI Components

extension MainTabView {
    private func presentWalletSelector() {
        guard walletSelectorEntry == nil else { return }
        let navigation = activeNavigation
        let parentPath = navigation.path
        let destination = WalletSelectorDestination(id: UUID())
        navigation.append(destination)
        let removalObserver = navigation.observeRemoval {
            guard walletSelectorEntry?.destination == destination else { return }
            finishWalletSelector()
        }
        walletSelectorEntry = WalletSelectorEntry(destination: destination, navigation: navigation, parentPath: parentPath, removalObserver: removalObserver)
    }

    private func dismissWalletSelector() {
        let entry = walletSelectorEntry
        finishWalletSelector()
        guard let entry else { return }
        var entryPath = entry.parentPath
        entryPath.append(entry.destination)
        guard entry.navigation.count >= entryPath.count else { return }
        var currentPrefix = entry.navigation.path
        currentPrefix.removeLast(currentPrefix.count - entryPath.count)
        guard currentPrefix == entryPath else { return }
        entry.navigation.binding.wrappedValue = entry.parentPath
    }

    private func finishWalletSelector() {
        guard walletSelectorEntry != nil || presenter.isPresentingWallets.wrappedValue else { return }
        let entry = walletSelectorEntry
        walletSelectorEntry = nil
        if let entry {
            entry.navigation.removeRemovalObserver(entry.removalObserver)
            if let context = rootModel.coldWalletContext(in: entry.navigation) {
                rootModel.cancelColdWalletContextForNavigationRemoval(id: context.id)
            }
        }
        rootModel.cancelCollectiveOnboarding(onlyOnboarding: true)
        presenter.isPresentingWallets.wrappedValue = false
    }

    private func walletSelector(navigation: NavigationPathState) -> some View {
        WalletSelectorScene(
            directory: directory,
            presentation: navigationState.walletPresentation,
            navigation: navigation,
            onDismiss: dismissWalletSelector,
            onSelectLocal: {
                rootModel.cancelColdWalletContext()
                rootModel.cancelCollectiveOnboarding()
                directory.cancel()
                navigationState.selectWalletPresentation(.local)
            },
            onSelectCollective: {
                let selection = WalletPresentation.collective(signerWalletId: $0, walletId: $1)
                guard navigationState.walletPresentation != selection else { return }
                rootModel.cancelColdWalletContext()
                rootModel.cancelCollectiveOnboarding()
                directory.cancel()
                navigationState.selectWalletPresentation(selection)
            },
            onRefresh: {
                await rootModel.lockManager.lockModel.waitUntilUnlocked()
                guard !Task.isCancelled else { return }
                await directory.refresh()
            },
        )
        .environment(rootModel)
    }

    private func openColdWallet(_ entry: Scenes.BsvColdWallet.Entry) {
        navigationState.settings.append(Scenes.BsvColdWallet(entry: entry))
    }

    private func refreshColdWalletRole() {
        guard let currentWallet = rootModel.currentWallet else {
            coldWalletRoleState = .failed
            return
        }
        do {
            coldWalletRoleState = try .ready(currentWallet, viewModelFactory.coldWalletRole(for: currentWallet))
        } catch {
            coldWalletRoleState = .failed
        }
    }

    private var coldWalletRole: BsvHomeBsvColdWalletRole? {
        guard case let .ready(classifiedWallet, role) = coldWalletRoleState, classifiedWallet == wallet else { return nil }
        return role
    }

    private var isCurrentWalletPresentation: Bool {
        navigationState.walletPresentation.signerWalletId == nil || navigationState.walletPresentation.signerWalletId == wallet.id
    }

    private func openProfile(_ destination: ProfileDestination) {
        if case let .collective(_, walletId) = navigationState.walletPresentation {
            guard multisigModel?.wallet(walletId) != nil else {
                model.isPresentingToastMessage = ToastMessage(title: directory.errorMessage ?? Localized.multisigErrorMissingRecord, image: "info.circle")
                return
            }
            let route: BsvMultisigRoute = switch destination {
            case .activity: .history(walletId)
            case .paymail: .paymail(walletId)
            case .receive: .receive(walletId)
            }
            navigationState.settings.binding.wrappedValue.append(route)
            return
        }
        guard isPersonalWalletActive else {
            model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailableDescription, image: "info.circle")
            return
        }
        switch destination {
        case .activity: navigationState.settings.append(WalletTabDestination.activity)
        case .paymail: navigationState.settings.append(WalletTabDestination.paymail)
        case .receive: sheet = .receive
        }
    }

    private func openCreateWalletIntro() {
        if navigationState.settings.append(WalletTabDestination.createIntro) {
            createIntroDepth = navigationState.settings.count
        }
    }

    private func startCreateWalletFlow() {
        guard createWalletFlow == nil else { return }
        let settingsPath = navigationState.settings
        let flow = CreateWalletFlow(
            model: viewModelFactory.createWalletScene(onComplete: { settingsPath.reset() }),
            onNavigate: { _ = settingsPath.append($0) },
        )
        createWalletFlow = flow
        settingsPath.append(flow.initialStep)
    }

    private func openCollectiveOnboarding(_ entry: CollectiveOnboardingModel.Entry) {
        if navigationState.selectedTab != .settings {
            onSelect(tab: .settings)
        }
        collectiveOnboarding?.cancel()
        let model = rootModel.makeCollectiveOnboarding(entry: entry, directory: directory, signerWalletId: navigationState.walletPresentation.signerWalletId ?? wallet.id)
        collectiveOnboarding = model
        collectiveProfileDepth = navigationState.settings.count + 1
        navigationState.settings.append(CollectiveProfileDestination(id: model.id))
    }

    private var settingsPathBinding: Binding<NavigationPath> {
        Binding(get: { navigationState.settings.path }, set: { path in
            if collectiveProfileDepth > 0, path.count < collectiveProfileDepth {
                collectiveOnboarding?.cancel()
                collectiveOnboarding = nil
                collectiveProfileDepth = 0
            }
            if createIntroDepth > 0, path.count <= createIntroDepth {
                createWalletFlow?.discard()
                createWalletFlow = nil
            }
            if inlineImportDepth > 0, path.count < inlineImportDepth {
                inlineImportModel?.cancel()
                inlineImportModel = nil
                inlineImportDepth = 0
            }
            navigationState.settings.binding.wrappedValue = path
        })
    }

    private func openImportInput(_ kind: ImportWalletInputKind) {
        inlineImportModel?.cancel()
        inlineImportModel = viewModelFactory.importWalletScene(onComplete: {})
        inlineImportDepth = navigationState.settings.count + 1
        let destination: WalletTabDestination = switch kind {
        case .phrase: .importPhrase
        case .privateKey: .importPrivateKey
        case .address: .importWatch
        }
        navigationState.settings.append(destination)
    }

    @ViewBuilder
    private func importInputScene(kind: ImportWalletInputKind) -> some View {
        if let inlineImportModel {
            ImportWalletInputScene(kind: kind, importModel: inlineImportModel, onNewWallet: { importedWallet = $0 }, onExistingWallet: {
                cancelProfileOnboarding()
                navigationState.settings.reset()
            })
        } else {
            unavailablePersonalAction
        }
    }

    private func cancelProfileOnboarding() {
        createWalletFlow?.discard()
        createWalletFlow = nil
        collectiveOnboarding?.cancel()
        collectiveOnboarding = nil
        collectiveProfileDepth = 0
        inlineImportModel?.cancel()
        inlineImportModel = nil
        inlineImportDepth = 0
    }

    private var tabBar: some View {
        HStack(spacing: .zero) {
            ForEach(TabItem.allCases) { tab in
                Button { onSelect(tab: tab) } label: {
                    VStack(spacing: Spacing.tiny) {
                        tab.image.resizable().scaledToFit()
                            .frame(width: Sizing.wowPay.navigationIcon, height: Sizing.wowPay.navigationIcon)
                            .opacity(navigationState.selectedTab == tab ? 1 : WowPayOpacity.navigationIcon)
                        HStack(spacing: Spacing.tiny) {
                            Text(tab.title)
                            if tab == .settings, model.transactions > 0 {
                                Text(model.transactions.formatted())
                                    .foregroundStyle(Colors.WowPay.onBrand)
                                    .padding(.horizontal, Spacing.tiny)
                                    .background(Colors.WowPay.brand, in: Capsule())
                            }
                        }
                        .font(.wowPay.navigation)
                    }
                    .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.minimumTapTarget)
                    .foregroundStyle(navigationState.selectedTab == tab ? Colors.WowPay.brand : Colors.WowPay.navigationInactive)
                    .padding(.vertical, Spacing.small)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("wowpay-tab-\(tab)")
                .accessibilityAddTraits(navigationState.selectedTab == tab ? .isSelected : [])
            }
        }
        .background(Colors.WowPay.surface)
        .overlay(alignment: .top) { Divider().overlay(Colors.WowPay.border) }
    }

    private func tabItem(_ title: String, _ image: Image) -> Label<Text, Image> {
        Label(
            title: { Text(title) },
            icon: { image },
        )
    }
}

// MARK: - Actions

extension MainTabView {
    private var selectedWalletName: String {
        if case let .collective(_, walletId) = navigationState.walletPresentation {
            return directory.wallets.first { $0.id == walletId }?.name ?? Localized.multisigWalletTitle
        }
        if case .collectiveOverview = navigationState.walletPresentation {
            return Localized.multisigTitle
        }
        return wallet.name.isEmpty ? Localized.Wallets.title : wallet.name
    }

    private var isDedicatedWallet: Bool {
        viewModelFactory.walletService.isMultisigIdentity(walletId: wallet.id.id)
    }

    private var isPersonalWalletActive: Bool {
        navigationState.walletPresentation == .local && !isDedicatedWallet
    }

    private var unavailablePersonalAction: some View {
        ContentUnavailableView(Localized.wowpayUnavailableDescription, systemImage: "info.circle")
            .accessibilityIdentifier("collectiveUnavailableAction")
    }

    private func collectiveOverviewContent(path: Binding<NavigationPath>) -> some View {
        Group {
            if let collectiveOverview {
                CollectiveOnboardingScene(model: collectiveOverview, path: path, header: AnyView(collectiveHeader), listContext: collectiveListContext)
            } else {
                LoadingView().onAppear {
                    collectiveOverview = rootModel.makeCollectiveOnboarding(entry: .list, directory: directory, signerWalletId: navigationState.walletPresentation.signerWalletId ?? wallet.id, lifetime: .overview)
                }
                .accessibilityIdentifier("collectiveRootLoading")
            }
        }
    }

    private var collectiveListContext: BsvMultisigListContext {
        let overview = directory.overview
        return BsvMultisigListContext(
            wallets: overview.wallets,
            proposals: overview.pendingProposals,
            pendingCount: overview.pendingCount,
            onOpenWallet: { openCollectiveWallet($0.id) },
            onOpenProposal: { openCollectiveWallet($0.walletId, proposalId: $0.id) },
            isLoading: directory.isLoading,
            errorMessage: directory.errorMessage,
            onRefresh: { await directory.refresh() },
            pendingWallets: overview.pendingWallets,
        )
    }

    private func openCollectiveWallet(_ walletId: String, proposalId: String? = nil) {
        guard let wallet = directory.wallets.first(where: { $0.id == walletId }),
              let signerId = wallet.localSignerWalletId,
              let signer = directory.identities(for: walletId).first(where: { $0.id.id == signerId }),
              proposalId == nil || directory.overview.pendingProposals.contains(where: { $0.id == proposalId && $0.walletId == walletId })
        else {
            model.isPresentingToastMessage = ToastMessage(title: Localized.multisigErrorMissingSigner, image: "info.circle")
            return
        }
        rootModel.cancelCollectiveOnboarding()
        navigationState.selectWalletPresentation(.collective(signerWalletId: signer.id, walletId: walletId))
        if let proposalId {
            let path = navigationState.selectedTab == .home ? navigationState.home : navigationState.wallet
            path.binding.wrappedValue.append(BsvMultisigRoute.proposal(proposalId))
        }
    }

    private func resumeCollectiveOverviewAfterWalletSelector() {
        guard scenePhase == .active,
              !presenter.isPresentingWallets.wrappedValue,
              !rootModel.isPresentingCreateWalletSheet,
              !rootModel.isPresentingImportWalletSheet,
              rootModel.isPresentingConnectorSheet == nil,
              sheet == nil, importedWallet == nil,
              portalPaymentPresenter.payment == nil,
              presenter.isPresentingAssetInput.wrappedValue == nil,
              presenter.isPresentingPayment.wrappedValue == nil,
              presenter.isPresentingPriceAlert.wrappedValue == nil,
              !presenter.isPresentingSupport.wrappedValue,
              let collectiveOverview,
              collectiveOverview.identityModal == nil,
              collectiveOverview.scanner.presentation == nil
        else { return }
        switch navigationState.walletPresentation {
        case .collectiveOverview: break
        case .local where isDedicatedWallet: break
        default: return
        }
        switch navigationState.selectedTab {
        case .home where navigationState.home.isEmpty: break
        case .wallet where navigationState.wallet.isEmpty: break
        default: return
        }
        rootModel.registerCollectiveOnboarding(collectiveOverview)
        collectiveOverview.start(afterUnlock: { await rootModel.lockManager.lockModel.waitUntilUnlocked() })
    }

    private func collectiveIdentityContext(for route: BsvMultisigRoute) -> BsvMultisigIdentityContext? {
        let remoteId = route.walletId(in: multisigModel)
        let options = directory.identities(for: remoteId).map { wallet in
            BsvMultisigIdentityOption(walletId: wallet.id.id, name: wallet.name, address: wallet.accounts.first { $0.chain == .bitcoinSv }?.address ?? "", isDedicated: viewModelFactory.walletService.isMultisigIdentity(walletId: wallet.id.id))
        }
        return BsvMultisigIdentityContext(options: options, selectedWalletId: navigationState.walletPresentation.signerWalletId?.id, isSelecting: isSelectingIdentity, onSelect: { id in
            selectCollectiveSigner(id, remoteWalletId: remoteId)
        })
    }

    private func selectCollectiveSigner(_ id: String, remoteWalletId: String?) {
        guard !isSelectingIdentity, let candidate = directory.identities(for: remoteWalletId).first(where: { $0.id.id == id }) else { return }
        do {
            if let remoteWalletId {
                try directory.selectSigner(candidate.id, for: remoteWalletId)
            }
        } catch {
            model.isPresentingToastMessage = ToastMessage(title: Localized.multisigErrorStorage, image: "info.circle")
            return
        }
        let previous = navigationState.walletPresentation
        let ticket = UUID()
        identitySelectionId = ticket
        isSelectingIdentity = true
        identitySelectionTask = Task {
            defer {
                if identitySelectionId == ticket {
                    identitySelectionTask = nil
                    identitySelectionId = nil
                    isSelectingIdentity = false
                }
            }
            await directory.refresh()
            guard !Task.isCancelled, identitySelectionId == ticket, navigationState.walletPresentation == previous else { return }
            rootModel.cancelCollectiveOnboarding()
            let selection: WalletPresentation = remoteWalletId.map { .collective(signerWalletId: candidate.id, walletId: $0) } ?? .collectiveOverview(signerWalletId: candidate.id)
            navigationState.selectWalletPresentation(selection)
        }
    }

    private func walletRoot(presentation: WalletNavigationPresentation, path: Binding<NavigationPath>) -> some View {
        walletContent(presentation: presentation, path: path)
            .environment(\.bsvMultisigRecipientScanner, recipientScanner(navigation: presentation == .home ? navigationState.home : navigationState.wallet))
    }

    @ViewBuilder
    private func walletContent(presentation: WalletNavigationPresentation, path: Binding<NavigationPath>) -> some View {
        if isCurrentWalletPresentation, case .failed = coldWalletRoleState {
            ContentUnavailableView {
                Label(Localized.coldWalletTitle, systemImage: "exclamationmark.triangle")
            } description: {
                Text(Localized.coldWalletStartupStorageError)
            } actions: {
                Button(Localized.Common.tryAgain, action: refreshColdWalletRole)
            }
        } else if isCurrentWalletPresentation, coldWalletRole == nil {
            LoadingView()
        } else if isCurrentWalletPresentation, coldWalletRole == .signer {
            let navigation = presentation == .home ? navigationState.home : navigationState.wallet
            walletNavigationContent(
                presentation: presentation,
                path: path,
                rootContent: AnyView(ColdWalletDashboard(wallet: wallet, navigation: navigation, tab: presentation == .home ? .home : .wallet, header: { onScan in AnyView(coldWalletHeader(onScan: onScan)) })),
            )
        } else if case .collectiveOverview = navigationState.walletPresentation {
            collectiveOverviewContent(path: path)
        } else if isDedicatedWallet, navigationState.walletPresentation == .local {
            collectiveOverviewContent(path: path)
        } else if case let .collective(_, walletId) = navigationState.walletPresentation {
            Group {
                if let multisigModel {
                    BsvMultisigWalletScene(model: multisigModel, walletId: walletId, presentation: .tab) {
                        collectiveHeader
                    }
                } else {
                    ScrollView {
                        VStack(spacing: Spacing.wowPay.card) {
                            collectiveHeader
                            if let error = directory.errorMessage {
                                Text(error).font(.wowPay.caption)
                                    .accessibilityIdentifier("collectiveRootError")
                                Button(Localized.Common.tryAgain) { Task { await directory.refresh() } }
                                    .buttonStyle(.wowPaySecondary())
                            } else {
                                LoadingView()
                                    .accessibilityIdentifier("collectiveRootLoading")
                            }
                        }
                        .padding(Spacing.wowPay.page)
                    }
                    .wowPayBackground()
                }
            }
            .bsvMultisigNavigation(model: multisigModel, navigationPath: path, identityContext: collectiveIdentityContext(for: .wallet(walletId)), identityContextForRoute: collectiveIdentityContext)
        } else {
            walletNavigationContent(presentation: presentation, path: path)
        }
    }

    private func walletNavigationContent(presentation: WalletNavigationPresentation, path: Binding<NavigationPath>, rootContent: AnyView? = nil) -> some View {
        WalletNavigationView(
            model: viewModelFactory.walletScene(
                wallet: wallet,
                isPresentingSelectedAssetInput: presenter.isPresentingAssetInput,
                isPresentingWallets: presenter.isPresentingWallets,
                onRequestScan: openPersonalScanner,
            ),
            selectedBalanceAsset: $selectedBalanceAsset,
            navigationPath: path,
            multisigModel: multisigModel,
            presentation: presentation,
            rootContent: rootContent,
            onPortalPayment: presentPortalPayment,
        )
    }

    private func coldWalletHeader(onScan: @escaping () -> Void) -> some View {
        WowPayBrandHeader(title: wallet.name.isEmpty ? Localized.Wallets.title : wallet.name, subtitle: "", mark: Image("wowpay-mark"), appearance: .walletSelection(onSelect: { presenter.isPresentingWallets.wrappedValue = true })) {
            Button(action: onScan) {
                Image("wowpay-scan").resizable().scaledToFit()
                    .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
            }
            .accessibilityLabel(Localized.Wallet.scan)
            .accessibilityIdentifier("coldWalletHeaderScan")
        }
    }

    private var collectiveHeader: some View {
        WowPayBrandHeader(title: selectedWalletName, subtitle: "", mark: Image("wowpay-mark"), appearance: .walletSelection(onSelect: { presenter.isPresentingWallets.wrappedValue = true })) {
            Button(action: openCollectiveScanner) {
                Image("wowpay-scan").resizable().scaledToFit()
                    .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
            }
            .accessibilityLabel(Localized.Wallet.scan)
            .accessibilityIdentifier("collectiveScan")
        }
    }

    @MainActor
    private func presentPortalPayment(source: PortalSwapPaymentSource, orderId: String, service: any BsvHomePortalSwapServiceProtocol) async throws {
        guard isPersonalWalletActive else { throw CancellationError() }
        let paymentWallet = wallet
        try await portalPaymentPresenter.present(
            wallet: paymentWallet,
            source: source,
            orderId: orderId,
            service: service,
            isActive: { viewModelFactory.walletSessionService.currentWalletId == paymentWallet.id && isPortalPaymentSourceActive(source) },
        )
    }

    private func isPortalPaymentSourceActive(_ source: PortalSwapPaymentSource) -> Bool {
        switch source {
        case .tab:
            navigationState.selectedTab == .swap
        case let .detail(id):
            navigationState.selectedTab == .wallet && navigationState.detailSwapDestination?.id == id && navigationState.wallet.count >= navigationState.detailSwapDepth
        case .acquisition:
            false
        }
    }

    private func cancelInactivePortalPayment() {
        if let source = portalPaymentPresenter.source, !isPortalPaymentSourceActive(source) {
            portalPaymentPresenter.cancel()
        }
    }

    private func onSelect(tab: TabItem) {
        if navigationState.selectedTab != tab {
            publicFunding?.dismiss()
            multisigScanner.cancel()
            presenter.cancelMultisigRequest()
        }
        if tab == .settings, navigationState.selectedTab == .settings {
            collectiveOnboarding?.cancel()
            collectiveOnboarding = nil
            createWalletFlow?.discard()
            createWalletFlow = nil
        }
        navigationState.select(tab: tab)
    }

    private var activeNavigation: NavigationPathState {
        switch navigationState.selectedTab {
        case .home: navigationState.home
        case .wallet: navigationState.wallet
        case .settings: navigationState.settings
        case .swap: navigationState.home
        }
    }

    private func consumeMultisigRequest() {
        guard let id = presenter.multisigRequest?.id,
              let request = presenter.takeMultisigRequest(id: id)
        else { return }
        if request.navigationMode == .replaceStack {
            publicFunding?.dismiss()
            activeNavigation.reset()
        }
        switch request.destination {
        case let .publicFunding(source):
            presentPublicFunding(.payment(source))
        case let .invitation(token):
            publicFunding?.dismiss()
            openCollectiveOnboarding(.join(invitation: token))
        case let .collectiveSend(signerWalletId, walletId, code):
            guard let signer = directory.identities(for: walletId).first(where: { $0.id.id == signerWalletId }),
                  navigationState.walletPresentation == .collective(signerWalletId: signer.id, walletId: walletId),
                  directory.model(for: signer.id)?.wallet(walletId) != nil
            else {
                model.isPresentingToastMessage = ToastMessage(title: Localized.multisigErrorMissingSigner, image: "info.circle")
                return
            }
            activeNavigation.append(CollectiveScannedSendDestination(signerWalletId: signerWalletId, walletId: walletId, code: code))
        }
    }

    private func presentPublicFunding(_ entry: BsvMultisigPublicFundingEntry) {
        publicFunding?.dismiss()
        if navigationState.selectedTab == .swap {
            navigationState.selectedTab = .home
            navigationState.previousSelectedTab = .home
        }
        let context = viewModelFactory.bsvPublicFundingContext(entry: entry, navigation: activeNavigation)
        publicFunding = context
        let id = context.id
        let exitObserver = navigationState.observeTabExit(tab: navigationState.selectedTab) { [weak context] in
            context?.dismiss()
        }
        context.present {
            navigationState.removeTabExitObserver(exitObserver)
            if publicFunding?.id == id {
                publicFunding = nil
            }
        }
    }

    @ViewBuilder
    private func publicFundingContent(_ destination: BsvPublicFundingDestination) -> some View {
        if let context = publicFunding, context.id == destination.id {
            BsvMultisigPublicFundingScene(sourceContextId: context.id, entry: context.entry, eligiblePayers: context.eligiblePayers, model: context.model, onSelectPayer: context.selectPayer, onRetryPayers: context.loadPayers, onOpenRecovery: {
                guard publicFunding?.id == context.id else { return }
                presentPublicFunding(.recovery)
            }, onClose: {
                guard publicFunding?.id == context.id else { return }
                context.dismiss()
            })
            .environment(context.navigation)
            .bsvMultisigPassword(authorization: context.authorization)
            .task(id: context.id) { context.loadPayers() }
        } else {
            Text(Localized.multisigErrorMissingRecord)
        }
    }

    @ViewBuilder
    private func collectiveScannedSend(_ destination: CollectiveScannedSendDestination, navigation: NavigationPathState) -> some View {
        if let signer = directory.identities(for: destination.walletId).first(where: { $0.id.id == destination.signerWalletId }),
           let feature = directory.model(for: signer.id)
        {
            BsvMultisigDestination(model: feature, route: .send(destination.walletId), initialRecipientCode: destination.code)
                .bsvMultisigNavigation(model: feature, navigationPath: navigation.binding, identityContext: collectiveIdentityContext(for: .send(destination.walletId)), identityContextForRoute: collectiveIdentityContext)
                .environment(navigation)
                .environment(\.bsvMultisigRecipientScanner, recipientScanner(navigation: navigation))
        } else {
            Text(Localized.multisigErrorMissingSigner)
        }
    }

    private func openPersonalScanner() {
        guard isPersonalWalletActive else { return }
        let selection = navigationState.walletPresentation
        let tab = navigationState.selectedTab
        let navigation = activeNavigation
        let path = navigation.path
        let walletId = wallet.id
        let isActive: @MainActor () -> Bool = {
            navigationState.walletPresentation == selection && navigationState.selectedTab == tab
                && navigation.path == path && scenePhase != .background && viewModelFactory.walletSessionService.currentWalletId == walletId
        }
        multisigScanner.present(navigation: navigation, isActive: isActive, scanType: .universal) { code in
            guard !navigationHandler.routeMultisigScan(code: code, isActive: isActive) else { return }
            Task { await navigationHandler.handle(code: code, isActive: isActive) }
        }
    }

    private func openCollectiveScanner() {
        guard case let .collective(_, walletId) = navigationState.walletPresentation,
              multisigModel?.wallet(walletId) != nil
        else {
            model.isPresentingToastMessage = ToastMessage(title: Localized.multisigWalletRequired, image: "info.circle")
            return
        }
        let selection = navigationState.walletPresentation
        let tab = navigationState.selectedTab
        let navigation = activeNavigation
        let path = navigation.path
        let isActive: @MainActor () -> Bool = {
            navigationState.walletPresentation == selection && navigationState.selectedTab == tab
                && navigation.path == path && scenePhase != .background && multisigModel?.wallet(walletId) != nil
        }
        multisigScanner.present(navigation: navigation, isActive: isActive, scanType: .universal) { code in
            _ = navigationHandler.routeMultisigScan(code: code, isActive: isActive)
        }
    }

    private func recipientScanner(navigation: NavigationPathState) -> BsvMultisigRecipientScanner {
        { completion in
            guard let feature = multisigModel else { return }
            let selection = navigationState.walletPresentation
            let tab = navigationState.selectedTab
            let path = navigation.path
            multisigScanner.present(navigation: navigation, isActive: {
                navigationState.walletPresentation == selection && navigationState.selectedTab == tab
                    && navigation.path == path && scenePhase != .background && multisigModel === feature
            }, scanType: .address, onCode: completion)
        }
    }

    private func onPaymentComplete() {
        presenter.isPresentingPayment.wrappedValue = nil
    }

    private func onSetPriceAlertComplete(message: String) {
        presenter.isPresentingPriceAlert.wrappedValue = nil
        model.isPresentingToastMessage = .priceAlert(message: message)
    }

    private func onComplete(type: SelectedAssetType) {
        switch type {
        case .receive, .stake, .earn, .buy, .sell:
            presenter.isPresentingAssetInput.wrappedValue = nil
        case let .send(type):
            switch type {
            case .nft:
                navigationState.select(tab: .settings)
                navigationState.settings.setPath([WalletTabDestination.activity])
            case .asset:
                break
            }
            presenter.isPresentingAssetInput.wrappedValue = nil
        case let .swap(fromAsset, _):
            Task {
                try await presenter.completeSwap(fromAsset: fromAsset, navigationState: navigationState)
            }
        }
    }
}
