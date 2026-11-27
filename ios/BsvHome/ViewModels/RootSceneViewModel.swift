// Copyright (c). Gem Wallet. All rights reserved.

import AppService
import Components
import Foundation
import protocol BsvHomeCore.BsvHomeAppStartServiceProtocol
import protocol BsvHomeCore.BsvHomeAppUpdateServiceProtocol
import protocol BsvHomeCore.BsvHomeDeviceServiceProtocol
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import protocol BsvHomeCore.BsvHomeTransactionStateServiceProtocol
import protocol BsvHomeCore.BsvHomeWalletSessionServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import LockManager
import Onboarding
import Primitives
import PrimitivesComponents
import SwiftUI
import WalletConnector

@Observable
@MainActor
final class RootSceneViewModel {
    private let onstartService: OnstartService
    private let appStartService: any BsvHomeAppStartServiceProtocol
    private let pushNotificationEnablerService: PushNotificationEnablerService
    private let appLifecycleService: AppLifecycleService
    private let navigationHandler: NavigationHandler
    private let appUpdateService: any BsvHomeAppUpdateServiceProtocol
    private let rateService: RateService
    private let toastPresenter: ToastPresenter
    private let deviceService: any BsvHomeDeviceServiceProtocol

    let observablePreferences: ObservablePreferences
    private let viewModelFactory: ViewModelFactory
    private let walletSessionService: any BsvHomeWalletSessionServiceProtocol
    let walletConnectorPresenter: WalletConnectorPresenter
    let lockManager: any LockWindowManageable

    var currentWallet: Wallet? {
        guard let id = walletSessionService.currentWalletId,
              let wallet = try? viewModelFactory.storeManager.walletStore.getWallet(id: id),
              BsvHomeRuntimeEnvironment.current.allowsWallet(wallet: wallet.map()) else { return nil }
        return BsvHomeRuntimeEnvironment.current.visibleWallet(wallet: wallet.map()).map()
    }

    var currentWalletId: WalletId? { walletSessionService.currentWalletId }
    var colorScheme: ColorScheme? { observablePreferences.appearance.colorScheme }
    var updateVersionAlertMessage: AlertMessage?

    var isPresentingToastMessage: ToastMessage? {
        get { toastPresenter.toastMessage }
        set { toastPresenter.toastMessage = newValue }
    }

    var isPresentingConnectorError: String? {
        get { walletConnectorPresenter.isPresentingError }
        set { walletConnectorPresenter.isPresentingError = newValue }
    }

    var isPresentingConnectorSheet: WalletConnectorSheetType? {
        get { walletConnectorPresenter.isPresentingSheet }
        set { walletConnectorPresenter.isPresentingSheet = newValue }
    }

    var isPresentingConnectorBar: Bool {
        get { walletConnectorPresenter.isPresentingConnectionBar }
        set { walletConnectorPresenter.isPresentingConnectionBar = newValue }
    }

    private(set) var presentedCreateWalletModel: CreateWalletModel?
    private(set) var presentedImportWalletModel: ImportWalletViewModel?
    private(set) var createCollectiveModel: CollectiveOnboardingModel?
    private(set) var importCollectiveModel: CollectiveOnboardingModel?
    private(set) var activeCollectiveOnboarding: CollectiveOnboardingModel?
    private var collectiveDirectory: CollectiveWalletDirectory?
    private(set) var activeColdWalletContext: BsvColdWalletContext?
    @ObservationIgnored private var coldWalletNavigation: NavigationPathState?
    @ObservationIgnored private var coldWalletRemovalObserver: UUID?

    var isPresentingCreateWalletSheet = false {
        didSet {
            if isPresentingCreateWalletSheet, presentedCreateWalletModel == nil {
                presentedCreateWalletModel = viewModelFactory.createWalletScene(onComplete: { [weak self] in self?.dismissCreateWallet() })
                createCollectiveModel = collectiveDirectory.map { makeCollectiveOnboarding(entry: .create, directory: $0) }
            } else if !isPresentingCreateWalletSheet {
                if let presentedCreateWalletModel {
                    cancelColdWalletContext(owner: ObjectIdentifier(presentedCreateWalletModel))
                }
                presentedCreateWalletModel?.cancel()
                createCollectiveModel?.cancel()
                presentedCreateWalletModel = nil
                createCollectiveModel = nil
            }
        }
    }

    var isPresentingImportWalletSheet = false {
        didSet {
            if isPresentingImportWalletSheet, presentedImportWalletModel == nil {
                presentedImportWalletModel = viewModelFactory.importWalletScene(onComplete: { [weak self] in self?.dismissImportWallet() })
                importCollectiveModel = collectiveDirectory.map { makeCollectiveOnboarding(entry: .importChoice, directory: $0) }
            } else if !isPresentingImportWalletSheet {
                if let presentedImportWalletModel {
                    cancelColdWalletContext(owner: ObjectIdentifier(presentedImportWalletModel))
                }
                presentedImportWalletModel?.cancel()
                importCollectiveModel?.cancel()
                presentedImportWalletModel = nil
                importCollectiveModel = nil
            }
        }
    }

    var toastOffset: CGFloat {
        UIDevice.current.userInterfaceIdiom == .phone ? .space32 + .space16 : .zero
    }

    init(
        observablePreferences: ObservablePreferences,
        walletConnectorPresenter: WalletConnectorPresenter,
        onstartService: OnstartService,
        appStartService: any BsvHomeAppStartServiceProtocol,
        pushNotificationEnablerService: PushNotificationEnablerService,
        appLifecycleService: AppLifecycleService,
        navigationHandler: NavigationHandler,
        lockWindowManager: any LockWindowManageable,
        viewModelFactory: ViewModelFactory,
        walletSessionService: any BsvHomeWalletSessionServiceProtocol,
        appUpdateService: any BsvHomeAppUpdateServiceProtocol,
        rateService: RateService,
        toastPresenter: ToastPresenter,
        deviceService: any BsvHomeDeviceServiceProtocol,
    ) {
        self.observablePreferences = observablePreferences
        self.walletConnectorPresenter = walletConnectorPresenter
        self.onstartService = onstartService
        self.appStartService = appStartService
        self.pushNotificationEnablerService = pushNotificationEnablerService
        self.appLifecycleService = appLifecycleService
        self.navigationHandler = navigationHandler
        lockManager = lockWindowManager
        self.viewModelFactory = viewModelFactory
        self.walletSessionService = walletSessionService
        self.appUpdateService = appUpdateService
        self.rateService = rateService
        self.toastPresenter = toastPresenter
        self.deviceService = deviceService
    }
}

// MARK: - Business Logic

extension RootSceneViewModel {
    func setup() {
        rateService.perform()
        if appUpdateService.isEnabled() {
            Task { await checkForUpdate() }
        }
        Task { await appLifecycleService.setup() }
        Task { await setupWallets() }
    }

    func onScenePhaseChanged(_: ScenePhase, _ newPhase: ScenePhase) {
        Task {
            await appLifecycleService.handleScenePhase(newPhase)
        }
    }

    func onPerpetualEnabledChanged(_: Bool, _: Bool) {
        Task {
            await appLifecycleService.updatePerpetualConnection()
        }
    }
}

// MARK: - Effects

extension RootSceneViewModel {
    func onChangeWalletId(preserveNavigation: Bool = false) {
        if !preserveNavigation {
            navigationHandler.resetNavigation()
        }
        guard let currentWallet else {
            Task { await appLifecycleService.updateWalletConnections() }
            return
        }
        setup(wallet: currentWallet)
    }

    func handleOpenUrl(_ url: URL) async {
        await navigationHandler.handle(url: url)
    }

    func configureCollectiveDirectory(_ directory: CollectiveWalletDirectory) {
        collectiveDirectory = directory
        if isPresentingCreateWalletSheet, createCollectiveModel == nil {
            createCollectiveModel = makeCollectiveOnboarding(entry: .create, directory: directory)
        }
        if isPresentingImportWalletSheet, importCollectiveModel == nil {
            importCollectiveModel = makeCollectiveOnboarding(entry: .importChoice, directory: directory)
        }
    }

    func makeCollectiveOnboarding(entry: CollectiveOnboardingModel.Entry, directory: CollectiveWalletDirectory, signerWalletId: WalletId? = nil, lifetime: CollectiveOnboardingModel.Lifetime = .onboarding, onIdentityComplete: @escaping @MainActor (Wallet, CollectiveOnboardingModel.IdentityCompletion) -> Void = { _, _ in }) -> CollectiveOnboardingModel {
        CollectiveOnboardingModel(entry: entry, directory: directory, factory: viewModelFactory, signerWalletId: signerWalletId, lifetime: lifetime, onIdentityComplete: onIdentityComplete)
    }

    func registerCollectiveOnboarding(_ model: CollectiveOnboardingModel) {
        guard activeCollectiveOnboarding !== model else { return }
        cancelColdWalletContext()
        activeCollectiveOnboarding?.cancel()
        activeCollectiveOnboarding = model
    }

    func cancelCollectiveOnboarding(onlyOnboarding: Bool = false) {
        if onlyOnboarding, activeCollectiveOnboarding?.lifetime == .overview {
            return
        }
        activeCollectiveOnboarding?.cancel()
        activeCollectiveOnboarding = nil
    }

    func coldWalletContext(in navigation: NavigationPathState) -> BsvColdWalletContext? {
        guard coldWalletNavigation === navigation, let context = activeColdWalletContext, !context.model.isCancelled else { return nil }
        return context
    }

    func registerColdWalletContext(_ context: BsvColdWalletContext, navigation: NavigationPathState? = nil) {
        guard activeColdWalletContext !== context else { return }
        cancelColdWalletContext()
        cancelCollectiveOnboarding()
        activeColdWalletContext = context
        if let navigation {
            coldWalletNavigation = navigation
            let id = context.id
            coldWalletRemovalObserver = navigation.observeRemoval { [weak self] in self?.cancelColdWalletContextForNavigationRemoval(id: id) }
        }
    }

    func cancelColdWalletContext(id: UUID) {
        guard activeColdWalletContext?.id == id else { return }
        cancelColdWalletContext()
    }

    func cancelColdWalletContext(owner: ObjectIdentifier) {
        guard activeColdWalletContext?.onboardingOwner == owner else { return }
        cancelColdWalletContext()
    }

    func cancelColdWalletContext(navigation: NavigationPathState) {
        guard coldWalletNavigation === navigation else { return }
        cancelColdWalletContext()
    }

    func cancelColdWalletContext() {
        takeColdWalletContext()?.cancel()
    }

    private func takeColdWalletContext() -> BsvColdWalletContext? {
        let context = activeColdWalletContext
        activeColdWalletContext = nil
        if let observer = coldWalletRemovalObserver {
            coldWalletNavigation?.removeRemovalObserver(observer)
        }
        coldWalletRemovalObserver = nil
        coldWalletNavigation = nil
        return context
    }

    func cancelColdWalletContextForNavigationRemoval(id: UUID) {
        guard activeColdWalletContext?.id == id else { return }
        takeColdWalletContext()?.cancelForNavigationRemoval()
    }

    func cancelColdWalletContextForNavigationRemoval(owner: ObjectIdentifier) {
        guard activeColdWalletContext?.onboardingOwner == owner else { return }
        takeColdWalletContext()?.cancelForNavigationRemoval()
    }

    func appColdWalletNavigation(in navigationState: NavigationStateManager) -> NavigationPathState? {
        guard activeColdWalletContext != nil, let navigation = coldWalletNavigation,
              navigation === navigationState.home || navigation === navigationState.wallet || navigation === navigationState.settings || navigation === navigationState.swap
        else { return nil }
        return navigation
    }

    func preservesColdWalletChange(_ walletId: WalletId?) -> Bool {
        guard let walletId, let context = activeColdWalletContext else { return false }
        do {
            return try context.consumeWalletSelection(walletId: walletId.id)
        } catch {
            isPresentingToastMessage = ToastMessage(title: Localized.coldWalletStartupStorageError, image: "exclamationmark.triangle")
            return false
        }
    }

    func currentWalletIsMultisigIdentity() throws -> Bool {
        guard let wallet = currentWallet, try viewModelFactory.coldWalletRole(for: wallet) == .none else { return false }
        return viewModelFactory.walletService.isMultisigIdentity(walletId: wallet.id.id)
    }

    func dismissCreateWallet() {
        isPresentingCreateWalletSheet = false
        requestPushPermissions()
    }

    func dismissImportWallet() {
        isPresentingImportWalletSheet = false
        requestPushPermissions()
    }
}

// MARK: - Private

extension RootSceneViewModel {
    private func setup(wallet: Wallet) {
        Task {
            for failure in await appStartService.setupWallet(wallet: wallet.map()) {
                debugLog("wallet start \(failure.step) failed: \(failure.message)")
            }
            await appLifecycleService.updateWalletConnections()
        }
    }

    private func setupWallets() async {
        await lockManager.lockModel.waitUntilUnlocked()
        await onstartService.setupWallets()
    }

    private func checkForUpdate() async {
        do {
            guard let release = try await appUpdateService.checkForUpdate() else { return }
            updateVersionAlertMessage = makeUpdateAlert(for: release)
        } catch {
            debugLog("checkForUpdate error: \(error)")
        }
    }

    private func makeUpdateAlert(for release: Release) -> AlertMessage {
        let skipAction = AlertAction(
            title: Localized.Common.skip,
            role: .cancel,
            action: { [appUpdateService] in
                do {
                    try appUpdateService.skip(version: release.version)
                } catch {
                    debugLog("skipRelease error: \(error)")
                }
            },
        )
        let updateAction = AlertAction(
            title: Localized.UpdateApp.action,
            isDefaultAction: true,
            action: {
                Task { @MainActor in
                    UIApplication.shared.open(AppUrl.page(.appStore))
                }
            },
        )
        let actions = release.upgradeRequired ? [updateAction] : [skipAction, updateAction]

        return AlertMessage(
            title: Localized.UpdateApp.title,
            message: Localized.UpdateApp.description(release.version),
            actions: actions,
        )
    }

    private func requestPushPermissions() {
        guard BsvHomeRuntimeEnvironment.current.usesBsvHomeHostedServices() else { return }
        Task {
            do {
                if try await pushNotificationEnablerService.requestPermissionsIfNotDetermined() {
                    try await deviceService.synchronizeIfNeeded()
                }
            } catch {
                debugLog("requestPushPermissions error: \(error)")
            }
        }
    }
}
