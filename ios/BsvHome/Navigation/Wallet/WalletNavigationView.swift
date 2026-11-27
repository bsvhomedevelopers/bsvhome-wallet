// Copyright (c). Gem Wallet. All rights reserved.

import Assets
import BsvMultisig
import Components
import enum BsvHomeCore.BsvHomePerpetualPositionAction
import protocol BsvHomeCore.BsvHomePortalSwapServiceProtocol
import BsvHomeCorePrimitives
import InfoSheet
import Localization
import MarketInsight
import NFT
import Perpetuals
import PriceAlerts
import Primitives
import PrimitivesComponents
import QRScanner
import Store
import Style
import SwiftUI
import Transactions
import Transfer
import WalletTab

struct WalletNavigationView: View {
    @Environment(\.navigationHandler) private var navigationHandler
    @Environment(\.navigationState) private var navigationState
    @Environment(\.navigationPresenter) private var presenter
    @Environment(\.viewModelFactory) private var viewModelFactory
    @State private var model: WalletSceneViewModel
    @State private var detailSwapModel: WowPayPortalSwapViewModel?
    @State private var homeReceiveAssetData: AssetData?
    @Environment(NavigationPathState.self) private var navigationOwner: NavigationPathState?
    @Environment(\.scenePhase) private var scenePhase
    @State private var homeSend = WowPaySendPresentation()
    @State private var isPresentingSendPaymail = false
    private let multisigModel: BsvMultisigViewModel?
    @Binding private var selectedBalanceAsset: WowPayAsset
    @Binding private var homeSwapAsset: WowPayAsset?
    private let presentation: WalletNavigationPresentation
    private let rootContent: AnyView?
    private let navigationPath: Binding<NavigationPath>
    private let onSelectHomeSwap: () -> Void
    private let onPortalPayment: @MainActor (PortalSwapPaymentSource, String, any BsvHomePortalSwapServiceProtocol) async throws -> Void

    init(
        model: WalletSceneViewModel,
        selectedBalanceAsset: Binding<WowPayAsset>,
        navigationPath: Binding<NavigationPath>,
        multisigModel: BsvMultisigViewModel?,
        presentation: WalletNavigationPresentation = .assets,
        homeSwapAsset: Binding<WowPayAsset?> = .constant(nil),
        onSelectHomeSwap: @escaping () -> Void = {},
        rootContent: AnyView? = nil,
        onPortalPayment: @escaping @MainActor (PortalSwapPaymentSource, String, any BsvHomePortalSwapServiceProtocol) async throws -> Void,
    ) {
        self.presentation = presentation
        self.rootContent = rootContent
        self.navigationPath = navigationPath
        self.multisigModel = multisigModel
        self.onSelectHomeSwap = onSelectHomeSwap
        self.onPortalPayment = onPortalPayment
        _model = State(initialValue: model)
        _selectedBalanceAsset = selectedBalanceAsset
        _homeSwapAsset = homeSwapAsset
    }

    var body: some View {
        ZStack {
            Group {
                if let rootContent {
                    rootContent
                } else {
                    switch presentation {
                    case .swap:
                        WowPaySwapScene(
                            model: viewModelFactory.portalSwapScene(onPayment: { orderId, service in
                                try await onPortalPayment(.tab, orderId, service)
                            }),
                            walletModel: model,
                            isActive: navigationState.selectedTab == .swap,
                            homeEntryAsset: $homeSwapAsset,
                        )
                        .wowPayTabStatusBarBackground()
                    case .assets:
                        AssetNavigationView(
                            model: viewModelFactory.assetScene(
                                wallet: model.wallet,
                                asset: Chain.bitcoinSv.asset,
                                isPresentingSelectedAssetInput: model.isPresentingSelectedAssetInput,
                            ),
                            multisigModel: multisigModel,
                            walletHeader: AnyView(walletHeader),
                        )
                    case .home:
                        WalletScene(
                            model: model,
                            selectedBalanceAsset: $selectedBalanceAsset,
                            presentation: presentation == .home ? .home : .assets,
                            onSelectAssets: { navigationState.select(tab: .wallet) },
                            onSelectSwap: onSelectHomeSwap,
                            onSelectAsset: navigationState.openAsset,
                            onSelectReceive: onSelectHomeReceive,
                            onSelectSend: onSelectHomeSend,
                            onSelectMultisig: navigationState.openMultisig,
                            onSelectColdWallet: { openColdWallet(entry: .home) },
                            multisigPendingCount: Int(multisigModel?.pendingCount ?? 0),
                        )
                        .wowPayTabStatusBarBackground()
                    }
                }
            }
            .opacity(model.isPresentingSearch ? 0 : 1)

            if model.isPresentingSearch {
                WalletSearchScene(
                    model: viewModelFactory.walletSearchScene(
                        wallet: model.wallet,
                        onDismissSearch: model.onToggleSearch,
                        onSelectAssetAction: navigationState.openAsset,
                        onAddToken: model.onSelectAddCustomToken,
                    ),
                )
                .transition(.opacity)
            }
        }
        .onChange(of: navigationState.wallet.count) { _, _ in navigationState.reconcileDetailSwapPath() }
        .onChange(of: navigationState.detailSwapDestination) { _, destination in
            if destination == nil {
                detailSwapModel?.onDisappear()
                detailSwapModel = nil
            }
        }
        .onChange(of: navigationState.walletTabReselected, model.onWalletTabReselected)
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                homeSend.dismiss()
            }
        }
        .bindQuery(model.walletQuery, model.assetsQuery, model.bannersQuery, model.fiatValuesQuery, model.collectionsModel.query)
        .bindQuery(model.fiatRatesQuery, model.perpetualBalanceQuery, model.fixedAssetsQuery, model.developmentAssetsQuery, model.bsvPriceUsdQuery, model.trxPriceUsdQuery, model.usdtPriceUsdQuery)
        .navigationBarTitleDisplayMode(.inline)
        .bsvMultisigNavigation(model: multisigModel, navigationPath: navigationPath)
        .navigationDestination(item: $homeReceiveAssetData) { assetData in
            ReceiveNavigationView(
                model: viewModelFactory.receiveScene(assetData: assetData, wallet: model.wallet),
                wallet: model.wallet,
                navigationTitle: Localized.wowpayReceivePaymentTitle,
            )
        }
        .navigationDestination(for: WowPaySendDestination.self) { destination in
            if homeSend.isCurrent(destination.id), let homeSendModel = homeSend.model {
                @Bindable var sendModel = homeSendModel
                @Bindable var confirmation = homeSend.confirmation
                WowPaySendScene(model: homeSendModel)
                    .sheet(isPresented: $sendModel.isPresentingScanner) {
                        ScanQRCodeNavigationStack(scanType: .address, action: homeSendModel.onHandleScan)
                    }
                    .navigationDestination(isPresented: $isPresentingSendPaymail) {
                        WowPayPaymailScene(model: viewModelFactory.paymailScene(walletId: model.wallet.id))
                    }
                    .navigationDestination(isPresented: $confirmation.isPresented) {
                        if let confirmation = homeSend.confirmation.model {
                            ConfirmTransferNavigationView(model: confirmation)
                                .id(confirmation.id)
                        }
                    }
            }
        }
        .navigationDestination(for: Scenes.Asset.self) {
            AssetNavigationView(
                model: viewModelFactory.assetScene(
                    wallet: model.wallet,
                    asset: $0.asset,
                    isPresentingSelectedAssetInput: model.isPresentingSelectedAssetInput,
                    onSwap: openDetailSwap,
                ),
                initialTab: $0.initialTab,
                multisigModel: multisigModel,
            )
        }
        .navigationDestination(for: PortalSwapDestination.self) { destination in
            if let detailSwapModel {
                WowPaySwapDetailScene(
                    model: detailSwapModel,
                    walletModel: model,
                    entryAsset: destination.entryAsset,
                    isActive: navigationState.selectedTab == .wallet && navigationState.detailSwapDestination == destination,
                )
                .id(destination.id)
            }
        }
        .navigationDestination(for: Scenes.NetworkAssets.self) { destination in
            NetworkAssetsScene(
                model: viewModelFactory.networkAssetsScene(
                    wallet: model.wallet,
                    chain: destination.chain,
                    onManageAssets: { model.onSelectManage(chains: [destination.chain]) },
                ),
            )
        }
        .navigationDestination(for: Scenes.Transaction.self) {
            TransactionNavigationView(
                model: viewModelFactory.transactionScene(
                    transaction: $0.transaction,
                    walletId: model.wallet.id,
                    onHeaderAction: onSelectTransactionHeaderAction,
                    onAddContact: { model.isPresentingSheet = .addContact($0) },
                ),
            )
        }
        .navigationDestination(for: Scenes.Collectible.self) {
            CollectibleScene(model: viewModelFactory.collectibleScene(wallet: model.wallet, assetData: $0.assetData, isPresentingSelectedAssetInput: model.isPresentingSelectedAssetInput))
        }
        .navigationDestination(for: Scenes.Collections.self) { _ in
            CollectionsSceneNavigationView(
                model: viewModelFactory.collectionsScene(wallet: model.wallet),
            )
        }
        .navigationDestination(for: Scenes.Collection.self) { scene in
            CollectionsScene(
                model: viewModelFactory.collectionScene(wallet: model.wallet, collectionId: scene.id, collectionName: scene.name),
            )
        }
        .navigationDestination(for: Scenes.UnverifiedCollections.self) { _ in
            CollectionsScene(
                model: viewModelFactory.unverifiedCollectionsScene(wallet: model.wallet),
            )
        }
        .navigationDestination(for: Scenes.Price.self) {
            ChartScene(
                model: viewModelFactory.chartScene(
                    asset: $0.asset,
                    walletId: model.wallet.id,
                    onSetPriceAlert: { presenter.isPresentingPriceAlert.wrappedValue = $0 },
                ),
            )
        }
        .navigationDestination(for: Scenes.Perpetuals.self) { _ in
            PerpetualsNavigationView(
                model: viewModelFactory.perpetualsScene(
                    wallet: model.wallet,
                    onSelectAssetType: { model.isPresentingSheet = .selectAsset($0, chains: []) },
                    onSelectAsset: navigationState.openAsset,
                    onSelectPortfolio: { model.isPresentingSheet = .portfolio(.perpetuals) },
                ),
            )
        }
        .navigationDestination(for: Scenes.AssetsResults.self) { destination in
            AssetsResultsScene(
                model: viewModelFactory.assetsResultsScene(
                    wallet: model.wallet,
                    request: WalletSearchRequest(
                        walletId: model.wallet.id,
                        searchBy: destination.searchQuery,
                        scope: destination.scope,
                    ),
                    title: destination.title ?? Localized.Assets.title,
                    onSelectAsset: navigationState.openAsset,
                ),
            )
        }
        .navigationDestination(for: Scenes.Perpetual.self) {
            PerpetualNavigationView(
                model: viewModelFactory.perpetualScene(
                    asset: $0.asset,
                    wallet: model.wallet,
                    onTransferData: { model.isPresentingSheet = .transferData($0) },
                    onPerpetualPosition: { model.isPresentingSheet = .perpetualPosition($0) },
                ),
                isPresentingSheet: $model.isPresentingSheet,
            )
        }
        .navigationDestination(for: Scenes.AssetPriceAlert.self) {
            AssetPriceAlertsScene(
                model: viewModelFactory.assetPriceAlertsScene(walletId: model.wallet.id, asset: $0.asset),
            )
        }
        .scanQRCodeSheet(isPresented: $model.isPresentingScanner, action: onScan)
        .sheet(item: $model.isPresentingSheet) { sheet in
            Group {
                switch sheet {
                case let .selectAsset(type, chains):
                    SelectAssetSceneNavigationStack(
                        model: viewModelFactory.selectAssetScene(
                            wallet: model.wallet,
                            selectType: type,
                            chains: chains,
                        ),
                    )
                case let .infoSheet(type):
                    InfoSheetScene(type: type)
                case let .transferData(data):
                    ConfirmTransferNavigationStack(
                        wallet: model.wallet,
                        transferData: data,
                        onComplete: model.onTransferComplete,
                    )
                case let .perpetualPosition(action):
                    PerpetualPositionNavigationStack(
                        positionAction: action,
                        wallet: model.wallet,
                        onComplete: { model.isPresentingSheet = nil },
                    )
                case .addAsset:
                    AddAssetNavigationStack(wallet: model.wallet)
                case let .portfolio(defaultType):
                    PortfolioScene(model: viewModelFactory.portfolioScene(wallet: model.wallet, defaultType: defaultType))
                case let .addContact(action):
                    AddContactNavigationView(action: action)
                case .swap:
                    SwapNavigationStack(wallet: model.wallet, onComplete: model.onTransferComplete)
                }
            }
            .id(sheet.id)
        }
        .safariSheet(url: $model.isPresentingUrl)
        .toast(message: $model.isPresentingToastMessage)
    }
}

// MARK: - Actions

extension WalletNavigationView {
    private func openColdWallet(entry: Scenes.BsvColdWallet.Entry) {
        guard let navigationOwner else {
            model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
            return
        }
        navigationOwner.append(Scenes.BsvColdWallet(walletId: model.wallet.id.id, entry: entry))
    }

    private func onSelectHomeReceive() {
        do {
            switch try viewModelFactory.coldWalletRole(for: model.wallet) {
            case .observer, .none:
                break
            case .signer:
                model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
                return
            }
        } catch {
            model.isPresentingToastMessage = ToastMessage(title: Localized.coldWalletStartupStorageError, image: "info.circle")
            return
        }
        homeReceiveAssetData = model.onSelectHomeReceive(asset: .bsv)
    }

    private func onSelectHomeSend() {
        do {
            switch try viewModelFactory.coldWalletRole(for: model.wallet) {
            case .observer:
                openColdWallet(entry: .send)
                return
            case .signer:
                model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
                return
            case .none:
                break
            }
        } catch {
            model.isPresentingToastMessage = ToastMessage(title: Localized.coldWalletStartupStorageError, image: "info.circle")
            return
        }
        guard let assetData = model.onSelectHomeSend(asset: .bsv) else { return }
        guard let navigationOwner else {
            model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
            return
        }
        do {
            let id = UUID()
            let senderId = model.wallet.id
            let tab = navigationState.selectedTab
            let selection = navigationState.walletPresentation
            let sendModel = try viewModelFactory.wowPaySendScene(
                wallet: model.wallet,
                assetData: assetData,
                onTransferAction: { data in
                    guard homeSend.isCurrent(id) else { return }
                    homeSend.confirmation.present(model: viewModelFactory.confirmTransferScene(wallet: model.wallet, data: data, onComplete: { homeSend.dismiss(id: id) }))
                },
                onManagePaymail: {
                    guard homeSend.isCurrent(id) else { return }
                    isPresentingSendPaymail = true
                },
                onPublicFunding: { source in
                    guard homeSend.isCurrent(id) else {
                        source.cancelPaymailPreparation()
                        return
                    }
                    homeSend.dismiss(id: id)
                    let parentPath = navigationOwner.path
                    presenter.requestMultisig(.publicFunding(source), navigationMode: .push, isActive: {
                        viewModelFactory.walletSessionService.currentWalletId == senderId && navigationState.selectedTab == tab
                            && navigationState.walletPresentation == selection && navigationOwner.path == parentPath && scenePhase != .background
                    })
                },
            )
            let exitObserver = navigationState.observeTabExit(tab: tab) {
                homeSend.dismiss(id: id)
            }
            homeSend.present(id: id, model: sendModel, navigation: navigationOwner, onRetire: {
                isPresentingSendPaymail = false
                navigationState.removeTabExitObserver(exitObserver)
            })
        } catch {
            model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
        }
    }

    @MainActor
    private func openDetailSwap(assetId: AssetId) {
        navigationState.openSwap(assetId: assetId)
        guard let id = navigationState.detailSwapDestination?.id else { return }
        detailSwapModel = viewModelFactory.portalSwapScene(onPayment: { orderId, service in
            try await onPortalPayment(.detail(id), orderId, service)
        })
    }

    private func onScan(_ code: String) {
        Task { await navigationHandler.handle(code: code) }
    }

    private var walletHeader: some View {
        WowPayBrandHeader(title: model.wallet.name.isEmpty ? Localized.Wallets.title : model.wallet.name, subtitle: "", mark: Image("wowpay-mark"), appearance: .walletSelection(onSelect: { model.isPresentingWallets.wrappedValue = true })) {
            Button(action: model.onSelectScanner) {
                Image("wowpay-scan").renderingMode(.template).resizable().scaledToFit()
                    .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                    .foregroundStyle(Colors.WowPay.brand)
            }
            .accessibilityLabel(Localized.Wallet.scan)
            .accessibilityIdentifier("scan")
        }
    }

    private func onSelectTransactionHeaderAction(_ action: TransactionHeaderAction) {
        Task {
            do {
                try await presenter.handleTransactionHeaderAction(
                    action,
                    wallet: model.wallet,
                    navigationState: navigationState,
                    nftDestination: navigationState.wallet,
                )
            } catch {
                model.isPresentingToastMessage = .error(Localized.Errors.errorOccurred)
            }
        }
    }
}
