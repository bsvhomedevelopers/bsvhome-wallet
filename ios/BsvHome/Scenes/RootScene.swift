// Copyright (c). Gem Wallet. All rights reserved.

import BsvColdWallet
import Components
import ConnectionStatusService
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import Onboarding
import Primitives
import Style
import SwiftUI

struct RootScene: View {
    private let connectionStatusObserver = AppResolver.main.services.connectionStatusObserver

    @Environment(\.viewModelFactory) private var viewModelFactory
    @Environment(\.navigationState) private var navigationState
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.navigationPresenter) private var presenter

    @State private var model: RootSceneViewModel
    @State private var directory: CollectiveWalletDirectory?

    init(model: RootSceneViewModel) {
        _model = State(initialValue: model)
    }

    private func coldWalletNavigation(_ content: AnyView, navigation: NavigationPathState) -> AnyView {
        AnyView(content.bsvColdWalletNavigation(model: model.coldWalletContext(in: navigation)?.model, navigation: navigation, onInitialPageBack: model.coldWalletContext(in: navigation)?.onInitialPageBack))
    }

    var body: some View {
        VStack {
            if let currentWallet = model.currentWallet {
                if let directory {
                    MainTabView(wallet: currentWallet, directory: directory)
                        .alertSheet($model.updateVersionAlertMessage)
                } else {
                    LoadingView()
                }
            } else {
                OnboardingScene(
                    isPresentingCreateWalletSheet: $model.isPresentingCreateWalletSheet,
                    isPresentingImportWalletSheet: $model.isPresentingImportWalletSheet,
                )
            }
        }
        .environment(model)
        .environment(\.connectionStatus, connectionStatusObserver.status)
        .onOpenURL { url in
            Task {
                await model.handleOpenUrl(url)
            }
        }
        .sheet(item: $model.isPresentingConnectorSheet) { type in
            WalletConnectorNavigationStack(
                type: type,
                presenter: model.walletConnectorPresenter,
            )
        }
        .sheet(isPresented: $model.isPresentingCreateWalletSheet) {
            if let creation = model.presentedCreateWalletModel {
                CreateWalletNavigationStack(model: creation, coldWalletDestination: { path in
                    AnyView(ColdWalletOnboardingDestination(entry: .create, owner: ObjectIdentifier(creation), navigation: path, onComplete: model.dismissCreateWallet))
                }, multisigWalletContent: { path in
                    AnyView(Group {
                        if let collective = model.createCollectiveModel {
                            CollectiveOnboardingScene(model: collective, path: path)
                        }
                    })
                }, onMultisigDismiss: { model.createCollectiveModel?.cancel() }, onColdWalletDismiss: { model.cancelColdWalletContextForNavigationRemoval(owner: ObjectIdentifier(creation)) }, navigationContent: coldWalletNavigation)
                    .environment(model)
            }
        }
        .sheet(isPresented: $model.isPresentingImportWalletSheet) {
            if let importing = model.presentedImportWalletModel {
                ImportWalletNavigationStack(model: importing, coldWalletDestination: { path in
                    AnyView(ColdWalletOnboardingDestination(entry: .recover, owner: ObjectIdentifier(importing), navigation: path, onComplete: model.dismissImportWallet))
                }, connectColdWalletDestination: { path in
                    AnyView(ColdWalletOnboardingDestination(entry: .connect, owner: ObjectIdentifier(importing), navigation: path, onComplete: model.dismissImportWallet))
                }, multisigWalletContent: { path in
                    AnyView(Group {
                        if let collective = model.importCollectiveModel {
                            CollectiveOnboardingScene(model: collective, path: path)
                        }
                    })
                }, onMultisigDismiss: { model.importCollectiveModel?.cancel() }, onColdWalletDismiss: { model.cancelColdWalletContextForNavigationRemoval(owner: ObjectIdentifier(importing)) }, navigationContent: coldWalletNavigation)
                    .environment(model)
            }
        }
        .alert(
            Localized.WalletConnect.brandName,
            presenting: $model.isPresentingConnectorError,
            actions: { _ in
                Button(
                    Localized.Common.done,
                    role: .none,
                    action: {},
                )
            },
            message: {
                Text(model.isPresentingConnectorError.valueOrEmpty)
            },
        )
        .taskOnce {
            let walletStore = viewModelFactory.storeManager.walletStore
            let authorization = viewModelFactory.bsvMultisigPasswordAuthorization()
            directory = CollectiveWalletDirectory(
                session: viewModelFactory.walletSessionService,
                loadWallets: { try walletStore.getWallets().map { $0.map() } },
                preferences: viewModelFactory.preferencesService,
                origin: viewModelFactory.wowPayPortalBaseURL,
                authorization: authorization,
                makeModel: { viewModelFactory.bsvMultisigScene(wallet: $0, authorization: authorization, cancelAuthorization: authorization.cancel(scopeId:)) },
            )
            if let directory {
                model.configureCollectiveDirectory(directory)
            }
            model.setup()
        }
        .task(id: collectiveRefreshSelection) {
            guard collectiveRefreshSelection.signerWalletId != nil, let directory else { return }
            var isInitialRefresh = true
            while !Task.isCancelled {
                await model.lockManager.lockModel.waitUntilUnlocked()
                guard !Task.isCancelled else { return }
                if isInitialRefresh || isCollectiveRootVisible {
                    await directory.refresh()
                    if case let .collective(signerWalletId, walletId) = collectiveRefreshSelection,
                       !Task.isCancelled
                    {
                        await directory.model(for: signerWalletId)?.refreshActivity(walletId)
                    }
                    isInitialRefresh = false
                }
                do { try await Task.sleep(for: .seconds(30)) }
                catch { return }
            }
        }
        .onChange(of: model.currentWalletId, initial: true) { previous, current in
            let preservesCollective = model.activeCollectiveOnboarding?.preservesWalletChange(current) == true
            let preservesCold = model.preservesColdWalletChange(current)
            let coldNavigation = preservesCold ? model.appColdWalletNavigation(in: navigationState) : nil
            if previous != current, !preservesCollective, !preservesCold {
                model.cancelColdWalletContext()
                model.cancelCollectiveOnboarding()
            }
            if current == nil {
                navigationState.onSettingsReset?()
                navigationState.onSettingsReset = nil
                directory?.cancel()
                model.cancelColdWalletContext()
                presenter.isPresentingWallets.wrappedValue = false
            }
            let isMultisigIdentity: Bool
            do {
                isMultisigIdentity = try model.currentWalletIsMultisigIdentity()
            } catch {
                isMultisigIdentity = false
                model.isPresentingToastMessage = ToastMessage(title: Localized.coldWalletStartupStorageError, image: "exclamationmark.triangle")
            }
            if isMultisigIdentity, let current {
                if navigationState.walletPresentation.signerWalletId == nil || previous != current {
                    navigationState.selectWalletPresentation(.collectiveOverview(signerWalletId: current))
                }
            } else if previous != current, (!preservesCollective && !preservesCold) || navigationState.walletPresentation.signerWalletId != current {
                navigationState.selectWalletPresentation(.local, preservingNavigation: coldNavigation)
            }
            model.onChangeWalletId(preserveNavigation: preservesCollective || coldNavigation != nil)
        }
        .onChange(of: navigationState.walletPresentation) { previous, current in
            if previous.signerWalletId != nil, previous != current {
                switch current {
                case .collective, .collectiveOverview: break
                case .local, .mpc: directory?.cancel()
                }
            }
            if case .mpc = current {
                navigationState.selectWalletPresentation(.local)
            }
        }
        .lockManaged(by: model.lockManager)
        .toast(
            isPresenting: $model.isPresentingConnectorBar,
            message: ToastMessage(
                title: "\(Localized.WalletConnect.brandName)...",
                image: SystemImage.network,
            ),
            offsetY: -model.toastOffset,
        )
        .toast(message: $model.isPresentingToastMessage, offsetY: -model.toastOffset)
        .onChange(of: scenePhase, model.onScenePhaseChanged)
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                directory?.cancel()
                model.cancelColdWalletContext()
                model.cancelCollectiveOnboarding()
                model.isPresentingCreateWalletSheet = false
                model.isPresentingImportWalletSheet = false
                presenter.isPresentingWallets.wrappedValue = false
            }
        }
        .onChange(of: model.observablePreferences.isPerpetualEnabled, model.onPerpetualEnabledChanged)
        .preferredColorScheme(model.colorScheme)
    }

    private var collectiveRefreshSelection: WalletPresentation {
        guard directory != nil, scenePhase != .background, navigationState.walletPresentation.signerWalletId != nil else { return .local }
        return navigationState.walletPresentation
    }

    private var isCollectiveRootVisible: Bool {
        switch navigationState.selectedTab {
        case .home: navigationState.home.isEmpty
        case .wallet: navigationState.wallet.isEmpty
        case .settings, .swap: false
        }
    }
}
