// Copyright (c). Gem Wallet. All rights reserved.

import Assets
import BsvColdWallet
import BsvMultisig
import Components
import Contacts
import FiatConnect
import Formatters
import Foundation
import func BsvHomeCore.bsvColdWalletRole
import class BsvHomeCore.BsvHomeAddAssetService
import class BsvHomeCore.BsvHomeAmountService
import class BsvHomeCore.BsvHomeApiClient
import class BsvHomeCore.BsvHomeAppStartService
import class BsvHomeCore.BsvHomeAppUpdateService
import class BsvHomeCore.BsvHomeAssetConfigService
import class BsvHomeCore.BsvHomeAssetDetailsService
import class BsvHomeCore.BsvHomeAssetDiscoveryService
import class BsvHomeCore.BsvHomeAssetSelectionService
import class BsvHomeCore.BsvHomeAssetsService
import class BsvHomeCore.BsvHomeAvatarService
import class BsvHomeCore.BsvHomeBalanceService
import class BsvHomeCore.BsvHomeBannerService
import class BsvHomeCore.BsvHomeBsvAddressPool
import enum BsvHomeCore.BsvHomeBsvColdWalletRole
import protocol BsvHomeCore.BsvHomeBsvColdWalletServiceProtocol
import protocol BsvHomeCore.BsvHomeBsvColdWalletStore
import protocol BsvHomeCore.BsvHomeBsvMultisigAuthorization
import struct BsvHomeCore.BsvHomeBsvMultisigFundingPayer
import protocol BsvHomeCore.BsvHomeBsvMultisigPublicFundingServiceProtocol
import enum BsvHomeCore.BsvHomeBsvMultisigPublicFundingSource
import protocol BsvHomeCore.BsvHomeBsvMultisigServiceProtocol
import class BsvHomeCore.BsvHomeChainService
import class BsvHomeCore.BsvHomeChartService
import class BsvHomeCore.BsvHomeCollectibleService
import struct BsvHomeCore.BsvHomeConfirmLoadOptions
import class BsvHomeCore.BsvHomeConfirmService
import class BsvHomeCore.BsvHomeConfirmTransferService
import class BsvHomeCore.BsvHomeContactService
import class BsvHomeCore.BsvHomeCurrencyService
import class BsvHomeCore.BsvHomeDeeplinkService
import class BsvHomeCore.BsvHomeDeveloperService
import class BsvHomeCore.BsvHomeDeviceService
import class BsvHomeCore.BsvHomeExplorerService
import class BsvHomeCore.BsvHomeFiatQuoteService
import class BsvHomeCore.BsvHomeFiatService
import class BsvHomeCore.BsvHomeKeystore
import class BsvHomeCore.BsvHomeManageContactService
import class BsvHomeCore.BsvHomeNameService
import class BsvHomeCore.BsvHomeNftService
import class BsvHomeCore.BsvHomeNodeService
import protocol BsvHomeCore.BsvHomeNotificationPermissions
import class BsvHomeCore.BsvHomeNotificationService
import class BsvHomeCore.BsvHomeNotificationsService
import class BsvHomeCore.BsvHomeOfflineSigningPasswordService
import class BsvHomeCore.BsvHomePaymailDestinationService
import protocol BsvHomeCore.BsvHomePaymailServiceProtocol
import struct BsvHomeCore.BsvHomePaymentRecipient
import class BsvHomeCore.BsvHomePaymentService
import class BsvHomeCore.BsvHomePerpetualDetailsService
import enum BsvHomeCore.BsvHomePerpetualPositionAction
import class BsvHomeCore.BsvHomePerpetualService
import protocol BsvHomeCore.BsvHomePortalSwapServiceProtocol
import protocol BsvHomeCore.BsvHomePortalTransferPasswordServiceProtocol
import class BsvHomeCore.BsvHomePortfolioService
import class BsvHomeCore.BsvHomePreferencesService
import class BsvHomeCore.BsvHomePriceAlertService
import class BsvHomeCore.BsvHomePriceService
import class BsvHomeCore.BsvHomeReceiveService
import class BsvHomeCore.BsvHomeRecentActivityService
import protocol BsvHomeCore.BsvHomeRecentActivityServiceProtocol
import class BsvHomeCore.BsvHomeRecipientService
import enum BsvHomeCore.BsvHomeRecipientType
import class BsvHomeCore.BsvHomeRewardsService
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import class BsvHomeCore.BsvHomeSearchService
import protocol BsvHomeCore.BsvHomeSecureStore
import class BsvHomeCore.BsvHomeSendPublicFundingPreparation
import class BsvHomeCore.BsvHomeSendService
import enum BsvHomeCore.BsvHomeServiceError
import class BsvHomeCore.BsvHomeServiceStatus
import class BsvHomeCore.BsvHomeSignMessageService
import class BsvHomeCore.BsvHomeSimulationFormatter
import class BsvHomeCore.BsvHomeStakeService
import class BsvHomeCore.BsvHomeStreamSubscriptionService
import protocol BsvHomeCore.BsvHomeSupportServiceProtocol
import class BsvHomeCore.BsvHomeSwapQuoteService
import class BsvHomeCore.BsvHomeSwapService
import protocol BsvHomeCore.BsvHomeTransactionCheckpoint
import struct BsvHomeCore.BsvHomeTransactionCheckpointData
import class BsvHomeCore.BsvHomeTransactionDetailsService
import class BsvHomeCore.BsvHomeTransactionsService
import class BsvHomeCore.BsvHomeTransactionStateService
import struct BsvHomeCore.BsvHomeTransferData
import class BsvHomeCore.BsvHomeWalletHomeService
import class BsvHomeCore.BsvHomeWalletPreferencesService
import class BsvHomeCore.BsvHomeWalletService
import class BsvHomeCore.BsvHomeWalletSessionService
import struct BsvHomeCore.SimulationResult
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import InAppNotifications
import LocalAuthentication
import Localization
import LockManager
import ManageWallets
import MarketInsight
import NFT
import Onboarding
import Perpetuals
import PriceAlerts
import Primitives
import PrimitivesComponents
import Recents
import Settings
import Stake
import Store
import Support
import Swap
import SwiftUI
import Transactions
import Transfer
import WalletConnector
import WalletConnectorService
import WalletTab

public struct ViewModelFactory: Sendable {
    let apiClient: BsvHomeApiClient
    let assetConfig: BsvHomeAssetConfigService
    let assetDiscoveryService: BsvHomeAssetDiscoveryService
    let assetsService: BsvHomeAssetsService
    let avatarService: BsvHomeAvatarService
    let bannerService: BsvHomeBannerService
    let balanceService: BsvHomeBalanceService
    let bsvAddressPool: BsvHomeBsvAddressPool
    let confirmService: BsvHomeConfirmService
    let contactService: BsvHomeContactService
    let manageContactService: BsvHomeManageContactService
    let deeplinkService: BsvHomeDeeplinkService
    let explorerService: BsvHomeExplorerService
    let fiatService: BsvHomeFiatService
    let gatewayService: GatewayService
    let hyperliquidObserverService: any PerpetualObservable
    let nameService: BsvHomeNameService
    let nftService: BsvHomeNftService
    let nodeService: BsvHomeNodeService
    let paymentService: BsvHomePaymentService
    let perpetualService: BsvHomePerpetualService
    let portfolioService: BsvHomePortfolioService
    let preferencesService: BsvHomePreferencesService
    let priceAlertService: BsvHomePriceAlertService
    let priceService: BsvHomePriceService
    let rewardsService: BsvHomeRewardsService
    let searchService: BsvHomeSearchService
    let simulationFormatter: BsvHomeSimulationFormatter
    let stakeService: BsvHomeStakeService
    let streamSubscriptionService: BsvHomeStreamSubscriptionService
    let swapService: BsvHomeSwapService
    let transactionStateService: BsvHomeTransactionStateService
    let transactionsService: BsvHomeTransactionsService
    let walletService: BsvHomeWalletService
    let walletSessionService: BsvHomeWalletSessionService
    let serviceStatusService: BsvHomeServiceStatus
    let appUpdateService: BsvHomeAppUpdateService
    let appStartService: BsvHomeAppStartService
    let inAppNotificationService: BsvHomeNotificationService

    let biometryService: any BiometryAuthenticatable
    let keystore: any Keystore
    let bsvHomeCoreKeystore: BsvHomeKeystore
    let observablePreferences: ObservablePreferences
    let recentAssetsService: BsvHomeRecentActivityService
    let amountService: BsvHomeAmountService
    let toastPresenter: ToastPresenter
    let walletPreferencesService: BsvHomeWalletPreferencesService
    let signMessageService: BsvHomeSignMessageService
    let developerService: BsvHomeDeveloperService
    let deviceService: BsvHomeDeviceService
    let notificationPermissions: any BsvHomeNotificationPermissions
    let storeManager: StoreManager
    let supportService: any BsvHomeSupportServiceProtocol
    let supportTyping: ObservableSupportTyping
    let paymailDestinationService: BsvHomePaymailDestinationService
    let phaseOneServices: PhaseOneServicesFactory
    let makePaymailService: @Sendable (String) throws -> any BsvHomePaymailServiceProtocol
    let makeWalletPaymailDestinationService: @Sendable (String) throws -> BsvHomePaymailDestinationService
    let makeTransferPasswordService: @Sendable (String) throws -> any BsvHomePortalTransferPasswordServiceProtocol
    let makeFixedTransferPasswordService: @Sendable (Wallet) throws -> any BsvHomePortalTransferPasswordServiceProtocol
    let makePortalSwapService: @Sendable () throws -> any BsvHomePortalSwapServiceProtocol
    let makeBsvMultisigService: @Sendable (Wallet, any BsvHomeBsvMultisigAuthorization) throws -> any BsvHomeBsvMultisigServiceProtocol
    let makeBsvPublicFundingService: @Sendable (Wallet, any BsvHomeBsvMultisigAuthorization) throws -> any BsvHomeBsvMultisigPublicFundingServiceProtocol
    let publicFundingPayers: @Sendable ([Wallet], BsvMultisigPublicFundingEntry) async throws -> [BsvHomeBsvMultisigFundingPayer]
    let makeBsvColdWalletService: @Sendable (BsvColdWalletPasswordAuthorization) throws -> any BsvHomeBsvColdWalletServiceProtocol
    let bsvColdWalletStore: any BsvHomeBsvColdWalletStore
    let bsvColdWalletPasswordStore: any BsvHomeSecureStore
    let wowPayPortalBaseURL: String

    @MainActor
    func authenticatePhaseOne() async throws {
        try await biometryService.authenticate(context: LAContext(), reason: Localized.Settings.Security.authentication)
    }

    @MainActor
    func bsvColdWalletContext(id: UUID = UUID(), walletId: String? = nil, entry: BsvColdWalletEntry = .home, onboardingOwner: ObjectIdentifier? = nil, onComplete: @escaping @MainActor () -> Void = {}) throws -> BsvColdWalletContext {
        let authorization = BsvColdWalletPasswordAuthorization(store: bsvColdWalletPasswordStore)
        let model = try BsvColdWalletViewModel(service: makeBsvColdWalletService(authorization), mode: .unified, walletId: walletId, initialPage: entry, cancelAuthorization: authorization.cancel(scopeId:), onComplete: onComplete)
        return BsvColdWalletContext(id: id, model: model, authorization: authorization, onboardingOwner: onboardingOwner)
    }

    func coldWalletRole(for wallet: Wallet) throws -> BsvHomeBsvColdWalletRole {
        try bsvColdWalletRole(wallet: wallet.map(), store: bsvColdWalletStore)
    }

    @MainActor
    func bsvMultisigPasswordAuthorization() -> BsvMultisigPasswordAuthorization {
        BsvMultisigPasswordAuthorization(makeService: makeFixedTransferPasswordService)
    }

    @MainActor
    public func bsvMultisigScene(wallet: Wallet, authorization: any BsvHomeBsvMultisigAuthorization, cancelAuthorization: @escaping @MainActor (String) -> Void) -> BsvMultisigViewModel {
        BsvMultisigViewModel(makeService: { try makeBsvMultisigService(wallet, authorization) }, fiatBalance: { satoshis in
            guard let value = Decimal(string: satoshis, locale: Locale(identifier: "en_US_POSIX")),
                  let price = try? storeManager.priceStore.getUsdPrice(for: Chain.bitcoinSv.asset.id),
                  price.isFinite, price > 0
            else { return nil }
            return CurrencyFormatter(currencyCode: "USD").balanceString(decimal: value / 100_000_000 * Decimal(price))
        }, walletName: { id in
            guard let walletId = try? WalletId.from(id: id) else { return nil }
            return try? storeManager.walletStore.getWallet(id: walletId)?.name
        }, syncPayer: { id in
            async let balance: Void = balanceService.update(walletId: id, assetIds: [Chain.bitcoinSv.asset.id.identifier])
            async let history: Void = transactionsService.syncWallet(walletId: id, assetId: Chain.bitcoinSv.asset.id.identifier)
            _ = try await (balance, history)
        }, cancelAuthorization: cancelAuthorization)
    }

    @MainActor
    func bsvPublicFundingContext(entry: BsvMultisigPublicFundingEntry, navigation: NavigationPathState) -> BsvPublicFundingContext {
        let authorization = bsvMultisigPasswordAuthorization()
        return BsvPublicFundingContext(entry: entry, navigation: navigation, authorization: authorization, loadCandidates: {
            let wallets = try storeManager.walletStore.getWallets()
            let payers = try await publicFundingPayers(wallets, entry)
            return (wallets, payers)
        }, makeModel: { payer in
            BsvMultisigPublicFundingViewModel(entry: entry, payer: payer.map(), makeService: {
                try makeBsvPublicFundingService(payer, authorization)
            }, syncPayer: { id in
                async let balance: Void = balanceService.update(walletId: id, assetIds: [Chain.bitcoinSv.asset.id.identifier])
                async let history: Void = transactionsService.syncWallet(walletId: id, assetId: Chain.bitcoinSv.asset.id.identifier)
                _ = try await (balance, history)
            }, cancelAuthorization: authorization.cancel(scopeId:))
        })
    }

    @MainActor
    public func paymailScene(walletId: WalletId) -> WowPayPaymailViewModel {
        WowPayPaymailViewModel(walletId: walletId.id, makeService: { try makePaymailService(walletId.id) })
    }

    @MainActor
    public func onlineTransferPasswordScene(walletId: WalletId) throws -> TransferPasswordViewModel {
        let wallet = try passwordManagementWallet(walletId: walletId)
        return TransferPasswordViewModel(mode: .management, walletName: wallet.name, makeService: { try makeTransferPasswordService(walletId.id) })
    }

    @MainActor
    public func offlineSigningPasswordScene(walletId: WalletId) throws -> TransferPasswordViewModel {
        let wallet = try passwordManagementWallet(walletId: walletId)
        return TransferPasswordViewModel(mode: .management, walletName: wallet.name, backend: .offline {
            try BsvHomeOfflineSigningPasswordService(session: walletSessionService, store: bsvColdWalletPasswordStore, wallet: wallet.map(), domain: .unifiedCold)
        })
    }

    @MainActor
    private func passwordManagementWallet(walletId: WalletId) throws -> Wallet {
        guard try walletSessionService.getCurrentWalletId() == walletId.id,
              let wallet = try storeManager.walletStore.getWallet(id: walletId)
        else { throw BsvHomeServiceError.Cancelled }
        return wallet
    }

    @MainActor
    public func portalSwapScene(entryMode: WowPayPortalSwapViewModel.EntryMode = .exchange, onPayment: @escaping @MainActor (String, any BsvHomePortalSwapServiceProtocol) async throws -> Void) -> WowPayPortalSwapViewModel {
        WowPayPortalSwapViewModel(makeService: makePortalSwapService, onPayment: onPayment, entryMode: entryMode)
    }

    @MainActor
    public func assetScene(
        wallet: Wallet,
        asset: Asset,
        isPresentingSelectedAssetInput: Binding<SelectedAssetInput?>,
        onSwap: ((AssetId) -> Void)? = nil,
    ) -> AssetSceneViewModel {
        AssetSceneViewModel(
            service: BsvHomeCore.BsvHomeAssetDetailsService(
                assets: assetsService,
                balances: balanceService,
                transactions: transactionsService,
                banners: bannerService,
                swap: swapService,
                explorer: explorerService,
                priceAlerts: priceAlertService,
                stream: streamSubscriptionService,
                deeplinks: deeplinkService,
                session: walletSessionService,
                addresses: bsvAddressPool,
            ),
            preferences: observablePreferences,
            input: AssetSceneInput(wallet: wallet, asset: asset),
            isPresentingSelectedAssetInput: isPresentingSelectedAssetInput,
            onSwap: onSwap,
        )
    }

    @MainActor
    public func portfolioScene(wallet: Wallet, defaultType: PortfolioType) -> PortfolioSceneViewModel {
        PortfolioSceneViewModel(wallet: wallet, service: portfolioService, preferences: observablePreferences, defaultType: defaultType)
    }

    @MainActor
    public func walletScene(
        wallet: Wallet,
        isPresentingSelectedAssetInput: Binding<SelectedAssetInput?>,
        isPresentingWallets: Binding<Bool>,
        onRequestScan: (@MainActor () -> Void)? = nil,
    ) -> WalletSceneViewModel {
        WalletSceneViewModel(
            service: walletHomeService(),
            observablePreferences: observablePreferences,
            collectionsModel: CollectionsViewModel(service: nftService, wallet: wallet),
            wallet: wallet,
            isPresentingSelectedAssetInput: isPresentingSelectedAssetInput,
            isPresentingWallets: isPresentingWallets,
            onRequestScan: onRequestScan,
        )
    }

    @MainActor
    public func notificationsScene() -> NotificationsViewModel {
        NotificationsViewModel(service: BsvHomeNotificationsService(device: deviceService, preferences: preferencesService, permissions: notificationPermissions))
    }

    @MainActor
    public func settingsScene() -> SettingsViewModel {
        SettingsViewModel(preferences: observablePreferences)
    }

    @MainActor
    public func preferencesScene() -> PreferencesViewModel {
        PreferencesViewModel(currencyModel: currencyScene(), preferences: observablePreferences)
    }

    @MainActor
    public func currencyScene() -> CurrencySceneViewModel {
        CurrencySceneViewModel(
            preferences: observablePreferences,
            service: BsvHomeCurrencyService(preferences: preferencesService, prices: priceService, device: deviceService),
        )
    }

    @MainActor
    public func aboutUsScene() -> AboutUsViewModel {
        AboutUsViewModel(preferences: observablePreferences, service: appUpdateService)
    }

    @MainActor
    public func chainListSettingsScene() -> ChainListSettingsViewModel {
        ChainListSettingsViewModel(service: BsvHomeChainService.newWithEnvironment(environment: .current))
    }

    @MainActor
    public func serviceStatusScene() -> ServiceStatusViewModel {
        ServiceStatusViewModel(service: serviceStatusService)
    }

    @MainActor
    public func priceAlertsScene() -> PriceAlertsSceneViewModel {
        PriceAlertsSceneViewModel(service: priceAlertService)
    }

    @MainActor
    public func assetPriceAlertsScene(walletId: WalletId, asset: Asset) -> AssetPriceAlertsViewModel {
        AssetPriceAlertsViewModel(service: priceAlertService, walletId: walletId, asset: asset)
    }

    @MainActor
    public func setPriceAlertScene(walletId: WalletId, asset: Asset, onComplete: StringAction) -> SetPriceAlertViewModel {
        SetPriceAlertViewModel(walletId: walletId, asset: asset, service: priceAlertService, onComplete: onComplete)
    }

    @MainActor
    public func inAppNotificationsScene() -> InAppNotificationsViewModel? {
        currentWallet(in: currentWallets()).map { InAppNotificationsViewModel(wallet: $0, service: inAppNotificationService) }
    }

    @MainActor
    public func perpetualsScene(
        wallet: Wallet,
        onSelectAssetType: @escaping (SelectAssetType) -> Void,
        onSelectAsset: @escaping (Asset) -> Void,
        onSelectPortfolio: @escaping () -> Void,
    ) -> PerpetualsSceneViewModel {
        PerpetualsSceneViewModel(
            wallet: wallet,
            service: perpetualService,
            observerService: hyperliquidObserverService,
            recentAssetsService: recentAssetsService,
            onSelectAssetType: onSelectAssetType,
            onSelectAsset: onSelectAsset,
            onSelectPortfolio: onSelectPortfolio,
        )
    }

    @MainActor
    public func collectionsScene(wallet: Wallet) -> CollectionsViewModel {
        CollectionsViewModel(service: nftService, wallet: wallet)
    }

    @MainActor
    public func collectionScene(wallet: Wallet, collectionId: String, collectionName: String) -> CollectionViewModel {
        CollectionViewModel(service: nftService, wallet: wallet, collectionId: collectionId, collectionName: collectionName)
    }

    @MainActor
    public func unverifiedCollectionsScene(wallet: Wallet) -> UnverifiedCollectionsViewModel {
        UnverifiedCollectionsViewModel(service: nftService, wallet: wallet)
    }

    @MainActor
    public func validatorSelectScene(
        currentValidator: DelegationValidator?,
        recommended: [DelegationValidator],
        validators: [DelegationValidator],
        selectValidator: @escaping (DelegationValidator) -> Void,
    ) -> ValidatorSelectSceneViewModel {
        ValidatorSelectSceneViewModel(
            service: stakeService,
            currentValidator: currentValidator,
            recommended: recommended,
            validators: validators,
            selectValidator: selectValidator,
        )
    }

    @MainActor
    public func walletSearchScene(
        wallet: Wallet,
        onDismissSearch: VoidAction,
        onSelectAssetAction: AssetAction,
        onAddToken: VoidAction,
    ) -> WalletSearchSceneViewModel {
        let service = assetSelectionService()
        return WalletSearchSceneViewModel(
            wallet: wallet,
            service: service,
            recentModel: RecentAssetsModel(
                walletId: wallet.id,
                types: RecentActivityType.allCases,
                allowedChains: service.filterChains(wallet: wallet.map()).map { Chain(core: $0) },
                service: recentAssetsService,
            ),
            onDismissSearch: onDismissSearch,
            onSelectAssetAction: onSelectAssetAction,
            onAddToken: onAddToken,
        )
    }

    @MainActor
    public func networkAssetsScene(wallet: Wallet, chain: Chain, onManageAssets: @escaping () -> Void) -> NetworkAssetsSceneViewModel {
        NetworkAssetsSceneViewModel(wallet: wallet, chain: chain, service: walletHomeService(), onManageAssets: onManageAssets)
    }

    private func walletHomeService() -> BsvHomeWalletHomeService {
        BsvHomeWalletHomeService(
            balances: balanceService,
            assets: assetsService,
            discovery: assetDiscoveryService,
            banners: bannerService,
            walletPreferences: walletPreferencesService,
            preferences: preferencesService,
            session: walletSessionService,
        )
    }

    private func assetSelectionService() -> BsvHomeAssetSelectionService {
        BsvHomeAssetSelectionService(
            search: searchService,
            balances: balanceService,
            priceAlerts: priceAlertService,
            recentActivity: recentAssetsService,
            preferences: preferencesService,
            perpetuals: perpetualService,
            session: walletSessionService,
            swap: swapService,
        )
    }

    @MainActor
    public func chartScene(
        asset: Asset,
        walletId: WalletId,
        onSetPriceAlert: @escaping (Asset) -> Void,
    ) -> ChartSceneViewModel {
        ChartSceneViewModel(
            service: BsvHomeCore.BsvHomeChartService(
                api: apiClient,
                price: priceService,
                preferences: preferencesService,
                priceAlerts: priceAlertService,
                explorer: explorerService,
            ),
            preferences: observablePreferences,
            assetModel: AssetViewModel(asset: asset),
            walletId: walletId,
            onSetPriceAlert: onSetPriceAlert,
        )
    }

    @MainActor
    public func transactionScene(
        transaction: TransactionExtended,
        walletId: WalletId,
        onHeaderAction: @escaping (TransactionHeaderAction) -> Void,
        onAddContact: @escaping (AddContactType) -> Void,
    ) -> TransactionSceneViewModel {
        TransactionSceneViewModel(
            transaction: transaction,
            walletId: walletId,
            service: BsvHomeCore.BsvHomeTransactionDetailsService(explorer: explorerService, preferences: preferencesService),
            onHeaderAction: onHeaderAction,
            onAddContact: onAddContact,
        )
    }

    @MainActor
    public func transactionsScene(wallet: Wallet, type: TransactionsRequestType) -> TransactionsViewModel {
        TransactionsViewModel(service: transactionsService, wallet: wallet, type: type)
    }

    @MainActor
    public func supportChatScene() -> SupportChatSceneViewModel {
        SupportChatSceneViewModel(service: supportService, typing: supportTyping)
    }

    @MainActor
    public func developerScene(walletId: WalletId) -> DeveloperViewModel {
        DeveloperViewModel(
            walletId: walletId,
            service: developerService,
            transactionStore: storeManager.transactionStore,
            assetStore: storeManager.assetStore,
            stakeStore: storeManager.stakeStore,
            bannerStore: storeManager.bannerStore,
            priceStore: storeManager.priceStore,
        )
    }

    @MainActor
    public func lockScene() -> LockSceneViewModel {
        LockSceneViewModel(service: biometryService)
    }

    @MainActor
    public func securityScene(onManagePassword: (@MainActor (SecurityViewModel.PasswordManagement) -> Void)? = nil) -> SecurityViewModel {
        SecurityViewModel(service: biometryService, preferences: observablePreferences, onManagePassword: onManagePassword)
    }

    @MainActor
    public func walletsScene(
        navigationPath: Binding<NavigationPath>,
        isPresentingCreateWalletSheet: Binding<Bool>,
        isPresentingImportWalletSheet: Binding<Bool>,
    ) -> WalletsSceneViewModel {
        let coldRole: (@MainActor (Wallet) throws -> BsvHomeBsvColdWalletRole)? = if BsvHomeRuntimeEnvironment.current == .wowPay {
            { try self.coldWalletRole(for: $0) }
        } else {
            nil
        }
        return WalletsSceneViewModel(
            navigationPath: navigationPath,
            walletService: walletService,
            preferences: observablePreferences,
            isPresentingCreateWalletSheet: isPresentingCreateWalletSheet,
            isPresentingImportWalletSheet: isPresentingImportWalletSheet,
            deletionWarning: multisigDeletionWarning,
            coldRole: coldRole,
        )
    }

    @MainActor
    public func walletDetailScene(navigationPath: Binding<NavigationPath>, wallet: Wallet, onOpenPairing: (@MainActor (Wallet) -> Void)? = nil, onOpenSigning: (@MainActor (Wallet) -> Void)? = nil) -> WalletDetailViewModel {
        let coldRole: (@MainActor (Wallet) throws -> BsvHomeBsvColdWalletRole)?
        let canOpenPairing: (@MainActor (Wallet) async throws -> Bool)?
        if BsvHomeRuntimeEnvironment.current == .wowPay {
            coldRole = { try self.coldWalletRole(for: $0) }
            canOpenPairing = { try await self.coldWalletPairingAvailable($0) }
        } else {
            coldRole = nil
            canOpenPairing = nil
        }
        return WalletDetailViewModel(
            navigationPath: navigationPath,
            wallet: wallet,
            service: walletService,
            preferences: observablePreferences,
            deletionWarning: multisigDeletionWarning,
            authenticateSecret: { [biometryService] in try await biometryService.authenticate(reason: Localized.bsvHdBackupTitle) },
            coldRole: coldRole,
            canOpenPairing: canOpenPairing,
            onOpenPairing: onOpenPairing,
            onOpenSigning: onOpenSigning,
        )
    }

    @MainActor
    private func coldWalletPairingAvailable(_ wallet: Wallet) async throws -> Bool {
        guard wallet.type != .view, wallet.accounts.contains(where: { $0.chain == .bitcoinSv }) else { return false }
        switch try await walletService.secretKind(walletId: wallet.id.id) {
        case .phrase, .recovery:
            _ = try await walletService.bsvHdPairing(walletId: wallet.id.id)
            return true
        case .privateKey, .none:
            return false
        }
    }

    @MainActor
    private func multisigDeletionWarning(_ wallet: Wallet) -> String? {
        do {
            let authorization = BsvMultisigPasswordAuthorization(makeService: makeFixedTransferPasswordService, isForeground: { false })
            return try makeBsvMultisigService(wallet, authorization).relatedMultisigCount(signerWalletId: wallet.id.id) > 0 ? Localized.multisigDeleteSignerWarning : nil
        } catch {
            return Localized.multisigDeleteSignerWarning
        }
    }

    @MainActor
    public func walletImageScene(wallet: Wallet) -> WalletImageViewModel {
        WalletImageViewModel(wallet: wallet, avatarService: avatarService)
    }

    @MainActor
    public func createWalletScene(onComplete: VoidAction, purpose: WalletOnboardingPurpose = .personal) -> CreateWalletModel {
        CreateWalletModel(service: walletService, preferences: observablePreferences, avatarService: avatarService, onComplete: onComplete, purpose: purpose)
    }

    @MainActor
    public func importWalletScene(onComplete: VoidAction, purpose: WalletOnboardingPurpose = .personal) -> ImportWalletViewModel {
        ImportWalletViewModel(
            service: walletService,
            keystore: bsvHomeCoreKeystore,
            authenticate: { [biometryService] in try await biometryService.authenticate(reason: Localized.bsvHdPreviewTitle) },
            appStartService: appStartService,
            preferences: observablePreferences,
            nameService: nameService,
            avatarService: avatarService,
            onComplete: onComplete,
            purpose: purpose,
        )
    }

    @MainActor
    public func contactsScene(mode: ContactsViewModel.Mode = .list) -> ContactsViewModel {
        ContactsViewModel(service: contactService, manageContact: manageContactScene, mode: mode)
    }

    @MainActor
    public func manageContactScene(mode: ManageContactViewModel.Mode) -> ManageContactViewModel {
        ManageContactViewModel(service: manageContactService, nameService: nameService, mode: mode)
    }

    @MainActor
    public func addAssetScene(wallet: Wallet) -> AddAssetSceneViewModel {
        AddAssetSceneViewModel(
            wallet: wallet,
            service: BsvHomeCore.BsvHomeAddAssetService(assets: assetsService, balances: balanceService, explorer: explorerService),
        )
    }

    @MainActor
    public func selectAssetScene(selectType: SelectAssetType, selectAssetAction: AssetAction = .none) -> SelectAssetViewModel? {
        currentWallet(in: currentWallets()).map { selectAssetScene(wallet: $0, selectType: selectType, selectAssetAction: selectAssetAction) }
    }

    @MainActor
    public func selectAssetScene(
        wallet: Wallet,
        selectType: SelectAssetType,
        selectAssetAction: AssetAction = .none,
        chains: [Chain] = [],
    ) -> SelectAssetViewModel {
        SelectAssetViewModel(
            wallet: wallet,
            selectType: selectType,
            service: assetSelectionService(),
            recentAssetsService: recentAssetsService,
            selectAssetAction: selectAssetAction,
            chains: chains,
        )
    }

    @MainActor
    public func assetsResultsScene(
        wallet: Wallet,
        request: WalletSearchRequest,
        title: String,
        onSelectAsset: @escaping (Asset) -> Void,
    ) -> AssetsResultsSceneViewModel {
        AssetsResultsSceneViewModel(
            wallet: wallet,
            service: assetSelectionService(),
            request: request,
            title: title,
            onSelectAsset: onSelectAsset,
        )
    }

    @MainActor
    public func confirmTransferScene(
        wallet: Wallet,
        data: BsvHomeTransferData,
        confirmTransferDelegate: TransferDataCallback.ConfirmTransferDelegate? = nil,
        simulation: SimulationResult? = nil,
        checkpoint: (any BsvHomeTransactionCheckpoint)? = nil,
        additionalDetails: [ListItemModel] = [],
        context: ConfirmTransferContext = .standard,
        onComplete: VoidAction,
    ) -> ConfirmTransferSceneViewModel {
        let confirm = confirmTransferService()
        let passwordAuthorization = data.chain == .bitcoinSv ? TransferPasswordAuthorization(makeService: makeTransferPasswordService) : nil
        let session = if let passwordAuthorization {
            confirm.sessionWithAuthorization(wallet: wallet.map(), transfer: data, simulation: simulation, checkpoint: checkpoint, walletSession: walletSessionService, authorization: passwordAuthorization)
        } else if let checkpoint {
            confirm.sessionWithCheckpoint(wallet: wallet.map(), transfer: data, simulation: simulation, checkpoint: checkpoint)
        } else {
            confirm.session(wallet: wallet.map(), transfer: data, simulation: simulation)
        }
        return ConfirmTransferSceneViewModel(
            request: ConfirmTransferRequest(
                data: data,
                simulation: simulation,
                delegate: confirmTransferDelegate,
            ),
            wallet: wallet,
            session: session,
            additionalDetails: additionalDetails,
            context: context,
            passwordAuthorization: passwordAuthorization,
            onComplete: { [toastPresenter] in
                Task { toastPresenter.present(.transfer(for: data.inputType)) }
                onComplete?()
            },
        )
    }

    public func broadcastPortalPayment(wallet: Wallet, transfer: BsvHomeTransferData, saved: BsvHomeTransactionCheckpointData, checkpoint: any BsvHomeTransactionCheckpoint) async throws -> String {
        try await confirmTransferService().broadcastSavedTransaction(wallet: wallet.map(), transfer: transfer, saved: saved, checkpoint: checkpoint)
    }

    private func confirmTransferService() -> BsvHomeConfirmTransferService {
        BsvHomeConfirmTransferService(
            confirm: confirmService,
            explorer: explorerService,
            names: nameService,
            assetConfig: assetConfig,
            signer: KeystoreTransactionSigner(keystore: keystore),
            password: BsvHomeCoreKeystorePassword(keystore: keystore),
            recentActivity: recentAssetsService,
            preferences: preferencesService,
        )
    }

    @MainActor
    public func perpetualScene(
        asset: Asset,
        wallet: Wallet,
        onTransferData: TransferDataAction,
        onPerpetualPosition: ((BsvHomePerpetualPositionAction) -> Void)?,
    ) -> PerpetualSceneViewModel {
        PerpetualSceneViewModel(
            wallet: wallet,
            asset: asset,
            service: BsvHomePerpetualDetailsService(perpetuals: perpetualService, transactions: transactionsService, preferences: preferencesService, session: walletSessionService),
            observerService: hyperliquidObserverService,
            onTransferData: onTransferData,
            onPerpetualPosition: onPerpetualPosition,
        )
    }

    @MainActor
    public func collectibleScene(wallet: Wallet, assetData: NFTAssetData, isPresentingSelectedAssetInput: Binding<SelectedAssetInput?>) -> CollectibleViewModel {
        CollectibleViewModel(
            wallet: wallet,
            assetData: assetData,
            service: BsvHomeCollectibleService(nfts: nftService, avatars: avatarService, explorer: explorerService),
            isPresentingSelectedAssetInput: isPresentingSelectedAssetInput,
        )
    }

    @MainActor
    public func rewardsScene(activateCode: String?) -> RewardsViewModel? {
        let wallets = currentWallets()
        return RewardsViewModel(
            service: rewardsService,
            wallets: wallets,
            currentWallet: currentWallet(in: wallets),
            activateCode: activateCode,
        )
    }

    private func currentWallets() -> [Wallet] {
        (try? storeManager.walletStore.getWallets()) ?? []
    }

    private func currentWallet(in wallets: [Wallet]) -> Wallet? {
        walletSessionService.currentWalletId.flatMap { walletId in wallets.first { $0.id == walletId } }
    }

    @MainActor
    public func chainSettingsScene(chain: Chain) -> ChainSettingsSceneViewModel {
        ChainSettingsSceneViewModel(chain: chain, service: gatewayService.chainSettingsService(nodes: nodeService, explorer: explorerService))
    }

    @MainActor
    public func receiveScene(assetData: AssetData, wallet: Wallet) -> ReceiveViewModel {
        ReceiveViewModel(assetData: assetData, wallet: wallet, service: receiveService())
    }

    @MainActor
    public func receiveScene(assetAddress: AssetAddress, wallet: Wallet) -> ReceiveViewModel {
        ReceiveViewModel(assetAddress: assetAddress, wallet: wallet, service: receiveService())
    }

    private func receiveService() -> BsvHomeReceiveService {
        BsvHomeReceiveService(balances: balanceService, assets: assetsService, addresses: bsvAddressPool)
    }

    @MainActor
    public func recipientScene(
        wallet: Wallet,
        asset: Asset,
        type: BsvHomeRecipientType,
        recipient: BsvHomePaymentRecipient? = .none,
        onRecipientDataAction: RecipientDataAction,
        onTransferAction: TransferDataAction,
    ) -> RecipientSceneViewModel {
        RecipientSceneViewModel(
            wallet: wallet,
            asset: asset,
            service: BsvHomeRecipientService(names: nameService, payments: paymentService, session: walletSessionService),
            nameService: nameService,
            type: type,
            recipient: recipient,
            onRecipientDataAction: onRecipientDataAction,
            onTransferAction: onTransferAction,
        )
    }

    @MainActor
    public func wowPaySendScene(
        wallet: Wallet,
        assetData: AssetData,
        onTransferAction: TransferDataAction,
        onManagePaymail: VoidAction = nil,
        onPublicFunding: (@MainActor (BsvHomeBsvMultisigPublicFundingSource) -> Void)? = nil,
    ) throws -> WowPaySendViewModel {
        let paymail = try makeWalletPaymailDestinationService(wallet.id.id)
        let service = BsvHomeSendService(
            recipient: BsvHomeRecipientService(names: nameService, payments: paymentService, session: walletSessionService),
            amount: amountService,
            paymail: paymail,
        )
        let onPreparedFunding: (@MainActor (BsvHomeSendPublicFundingPreparation) -> Void)? = if let onPublicFunding {
            { preparation in
                onPublicFunding(.paymail(service: service, preparation: preparation))
            }
        } else {
            nil
        }
        return WowPaySendViewModel(
            wallet: wallet,
            assetData: assetData,
            service: service,
            nameService: nameService,
            prepareBsvAddresses: {
                guard assetData.asset.chain == .bitcoinSv else { return }
                _ = try await bsvAddressPool.prepare(walletId: wallet.id.id)
                _ = try await bsvAddressPool.sync(walletId: wallet.id.id)
                try await balanceService.update(walletId: wallet.id.id, assetIds: [assetData.asset.id.identifier])
            },
            feeLoader: { data in
                try await confirmTransferService()
                    .session(wallet: wallet.map(), transfer: data, simulation: nil)
                    .load(options: BsvHomeConfirmLoadOptions(
                        feeSelection: .priority(priority: data.defaultFeePriority()),
                        feeAssetId: nil,
                    ))
            },
            onTransferAction: onTransferAction,
            onManagePaymail: onManagePaymail,
            onPublicFunding: onPreparedFunding,
        )
    }

    @MainActor
    public func amountScene(
        input: AmountInput,
        wallet: Wallet,
        onTransferAction: TransferDataAction,
    ) -> AmountSceneViewModel {
        AmountSceneViewModel(
            input: input,
            wallet: wallet,
            service: amountService,
            onTransferAction: onTransferAction,
        )
    }

    @MainActor
    public func fiatScene(
        assetAddress: AssetAddress,
        wallet: Wallet,
        type: FiatQuoteType = .buy,
        amount: Int? = nil,
    ) -> FiatSceneViewModel {
        FiatSceneViewModel(
            service: BsvHomeFiatQuoteService(fiat: fiatService, balances: balanceService, session: walletSessionService),
            assetAddress: assetAddress,
            wallet: wallet,
            type: type,
            amount: amount,
        )
    }

    @MainActor
    public func swapScene(
        input: SwapInput,
        onSwap: @escaping (BsvHomeTransferData) -> Void,
    ) -> SwapSceneViewModel {
        SwapSceneViewModel(
            service: BsvHomeSwapQuoteService(
                swap: swapService,
                preferences: preferencesService,
                balances: balanceService,
                stream: streamSubscriptionService,
                session: walletSessionService,
            ),
            input: input,
            onSwap: onSwap,
        )
    }

    @MainActor
    public func stakeScene(
        wallet: Wallet,
        chain: Chain,
    ) -> StakeSceneViewModel {
        StakeSceneViewModel(
            wallet: wallet,
            chain: StakeChain(rawValue: chain.rawValue)!, // Expected Only StakeChain accepted.
            service: stakeService,
        )
    }

    @MainActor
    public func earnScene(
        wallet: Wallet,
        asset: Asset,
    ) -> EarnSceneViewModel {
        EarnSceneViewModel(
            wallet: wallet,
            asset: asset,
            service: stakeService,
        )
    }

    @MainActor
    public func delegationScene(
        wallet: Wallet,
        delegation: Delegation,
        asset: Asset,
        validators: [DelegationValidator],
        onAmountInputAction: AmountInputAction,
        onTransferAction: TransferDataAction,
    ) -> DelegationSceneViewModel {
        DelegationSceneViewModel(
            wallet: wallet,
            model: DelegationViewModel(service: stakeService, delegation: delegation, asset: asset, formatter: .auto, currencyCode: stakeService.getCurrency()),
            asset: asset,
            service: stakeService,
            validators: validators,
            onAmountInputAction: onAmountInputAction,
            onTransferAction: onTransferAction,
        )
    }

    @MainActor
    public func signMessageScene(
        payload: SignMessagePayload,
        confirmTransferDelegate: @escaping TransferDataCallback.ConfirmTransferDelegate,
    ) -> SignMessageSceneViewModel {
        SignMessageSceneViewModel(
            service: signMessageService,
            payload: payload,
            confirmTransferDelegate: confirmTransferDelegate,
        )
    }
}
