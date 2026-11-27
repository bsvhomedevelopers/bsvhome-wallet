// Copyright (c). Gem Wallet. All rights reserved.

import AppService
import ConnectionStatusService
import Foundation
import BsvHomeCore
import protocol BsvHomeCore.BsvHomeBalanceServiceProtocol
import protocol BsvHomeCore.BsvHomeFiatServiceProtocol
import protocol BsvHomeCore.BsvHomeNftServiceProtocol
import protocol BsvHomeCore.BsvHomePortfolioServiceProtocol
import protocol BsvHomeCore.BsvHomePriceServiceProtocol
import class BsvHomeCore.BsvHomeRecentActivityService
import protocol BsvHomeCore.BsvHomeStakeServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Keychain
import Localization
import NativeProviderService
import Primitives
import PrimitivesComponents
import Store
import StreamService
import WalletConnector
import WalletConnectorService
import WebSocketClient

struct ServicesFactory {
    func makeServices(storages: AppResolver.Storages, navigation: NavigationStateManager) -> AppResolver.Services {
        let storeManager = storages.storeManager
        let securePreferences = SecurePreferences()
        let preferencesStore = BsvHomeCorePreferencesStore.application()
        let preferencesService = BsvHomeCore.BsvHomePreferencesService(store: preferencesStore)
        let observablePreferences = ObservablePreferences(preferencesService: preferencesService)
        let nodeService = BsvHomeNodeService(store: BsvHomeCoreNodeStore(store: storeManager.nodeStore), preferences: preferencesStore)
        let nativeProvider = NativeProvider()
        let portalOrigin = Bundle.main.object(forInfoDictionaryKey: "WowPayPortalOrigin") as? String ?? ""
        let portalBaseURL = portalOrigin + "/api/portal"
        let deviceKeyService = BsvHomeCore.BsvHomeDeviceKeyService(store: BsvHomeCoreSecurePreferencesStore(namespace: "gateway"))
        let deviceRegistrationClient = Self.makeDeviceApiClient(provider: nativeProvider, deviceKey: deviceKeyService)

        let bsvHomeCoreWalletStore = BsvHomeCoreWalletStore(store: storeManager.walletStore)
        let walletPreferencesService = BsvHomeCore.BsvHomeWalletPreferencesService(store: BsvHomeCoreWalletPreferencesStore())
        let devicePlatform = MainActor.assumeIsolated { BsvHomeCoreDevicePlatform(preferencesService: preferencesService, deviceKeyService: deviceKeyService, securePreferences: securePreferences) }
        let deviceService = BsvHomeCore.BsvHomeDeviceService.newWithEnvironment(
            api: deviceRegistrationClient,
            subscriptions: BsvHomeCore.BsvHomeSubscriptionService(api: deviceRegistrationClient, store: bsvHomeCoreWalletStore),
            walletStore: bsvHomeCoreWalletStore,
            platform: devicePlatform,
            preferences: preferencesService,
            environment: .current,
        )
        let deviceApiClient = Self.makeDeviceApiClient(provider: nativeProvider, deviceKey: deviceKeyService)
        deviceApiClient.setDeviceSyncPreflight(device: deviceService)

        let connectionService = BsvHomeCore.BsvHomeConnectionService()
        let connectionStatusObserver = ConnectionStatusObserver(
            connectionService: connectionService,
            monitors: [
                InternetConnectionMonitor(connectionService: connectionService),
            ],
        )
        let apiClient = BsvHomeCore.BsvHomeApiClient(provider: nativeProvider)
        let staticApiClient = BsvHomeCore.BsvHomeStaticApiClient(provider: nativeProvider)
        let priceService = BsvHomeCore.BsvHomePriceService(
            store: BsvHomeCorePriceStore(priceStore: storeManager.priceStore, fiatRateStore: storeManager.fiatRateStore),
            preferences: preferencesService,
        )
        let bsvHomeCoreAssetStore = BsvHomeCoreAssetStore(assetStore: storeManager.assetStore, balanceStore: storeManager.balanceStore)
        let bsvHomeCoreFileStore = BsvHomeCoreFileStore()
        let bsvHomeCoreAddressStore = BsvHomeCoreAddressStore(store: storeManager.addressStore)
        let bsvHomeCoreBannerStore = BsvHomeCoreBannerStore(store: storeManager.bannerStore)
        let bsvHomeCoreNotificationStore = BsvHomeCoreNotificationStore(store: storeManager.inAppNotificationStore)
        let gatewayService = GatewayService(
            provider: nativeProvider,
            preferences: BsvHomeCorePreferencesStore(namespace: "gateway"),
            securePreferences: BsvHomeCoreSecurePreferencesStore(namespace: "gateway"),
            environment: .current,
        )
        let walletSessionService = BsvHomeCore.BsvHomeWalletSessionService.newWithEnvironment(store: BsvHomeCoreWalletSessionStore(store: preferencesStore), wallets: bsvHomeCoreWalletStore, environment: .current)
        let assetsService = gatewayService.assetsService(api: apiClient, store: bsvHomeCoreAssetStore, price: priceService, preferences: preferencesService, session: walletSessionService)
        let scanConfiguration = URLSessionConfiguration.default
        scanConfiguration.timeoutIntervalForRequest = Config().scanTimeout()
        let scanService = BsvHomeCore.BsvHomeScanService(
            api: Self.makeDeviceApiClient(
                provider: NativeProvider(session: URLSession(configuration: scanConfiguration)),
                deviceKey: deviceKeyService,
            ),
        )
        let paymentService = BsvHomeCore.BsvHomePaymentService(provider: nativeProvider)
        let transactionSimulationService = BsvHomeSimulationService(provider: nativeProvider, preferences: preferencesStore)
        let serviceStatusConfiguration = URLSessionConfiguration.default
        serviceStatusConfiguration.timeoutIntervalForRequest = serviceStatusTimeout()
        let serviceStatusService = BsvHomeCore.BsvHomeServiceStatus.newWithEnvironment(
            provider: NativeProvider(session: URLSession(configuration: serviceStatusConfiguration)),
            environment: .current,
        )
        let recentAssetsService = BsvHomeRecentActivityService(store: BsvHomeCoreRecentActivityStore(store: storeManager.recentActivityStore), session: walletSessionService)
        let explorerService = BsvHomeCore.BsvHomeExplorerService(preferences: preferencesService)
        let bsvAddressPool = gatewayService.bsvAddressPool(
            walletStore: bsvHomeCoreWalletStore,
            store: BsvHomeCoreBsvAddressStore(store: storeManager.bsvAddressPoolStore),
            keystore: storages.keystore.bsvHomeKeystore,
            password: BsvHomeCoreKeystorePassword(keystore: storages.keystore),
        )
        let walletService = BsvHomeCore.BsvHomeWalletService(
            keystore: storages.keystore.bsvHomeKeystore,
            password: BsvHomeCoreKeystorePassword(keystore: storages.keystore),
            store: bsvHomeCoreWalletStore,
            session: walletSessionService,
            appPreferences: preferencesService,
            files: bsvHomeCoreFileStore,
            preferences: walletPreferencesService,
            explorer: explorerService,
            addresses: bsvHomeCoreAddressStore,
            localizer: BsvHomeCoreLocalizer(),
            bsvAddresses: bsvAddressPool,
        )
        let avatarService = BsvHomeCore.BsvHomeAvatarService(wallets: bsvHomeCoreWalletStore, files: bsvHomeCoreFileStore, provider: nativeProvider)
        let webSocket = Self.makeWebSocket(deviceKeyService: deviceKeyService, reconnection: connectionService)
        let bsvHomeCorePriceAlertStore = BsvHomeCorePriceAlertStore(store: storeManager.priceAlertStore)
        let bsvHomeCoreBalanceStore = BsvHomeCoreBalanceStore(store: storeManager.balanceStore)
        let streamSubscriptionService = BsvHomeCore.BsvHomeStreamSubscriptionService(
            balances: bsvHomeCoreBalanceStore,
            alerts: bsvHomeCorePriceAlertStore,
            connection: BsvHomeCoreStreamConnection(webSocket: webSocket),
        )
        let balanceService = gatewayService.balanceService(
            walletStore: bsvHomeCoreWalletStore,
            assetStore: bsvHomeCoreAssetStore,
            store: bsvHomeCoreBalanceStore,
            assets: assetsService,
            stream: streamSubscriptionService,
            addresses: bsvAddressPool,
        )
        let stakeService = gatewayService.stakeService(
            staticApi: staticApiClient,
            store: BsvHomeCoreStakeStore(store: storeManager.stakeStore),
            addressStore: bsvHomeCoreAddressStore,
            explorer: explorerService,
            preferences: preferencesService,
            session: walletSessionService,
        )
        let nftService = BsvHomeCore.BsvHomeNftService(api: deviceApiClient, store: BsvHomeCoreNftStore(store: storeManager.nftStore), session: walletSessionService)
        let transactionStateStore = BsvHomeCoreTransactionStateStore(store: storeManager.transactionStore, walletStore: storeManager.walletStore)
        let transactionStateService = gatewayService.transactionStateService(
            store: transactionStateStore,
            assets: assetsService,
            balance: balanceService,
            stake: stakeService,
            nft: nftService,
        )
        let transactionsService = gatewayService.transactionsService(
            api: deviceApiClient,
            assets: assetsService,
            store: BsvHomeCoreTransactionStore(store: storeManager.transactionStore),
            addressStore: bsvHomeCoreAddressStore,
            walletPreferences: walletPreferencesService,
            preferences: preferencesService,
            session: walletSessionService,
            transactionStatus: BsvHomeCoreTransactionStatusService(service: transactionStateService),
            addresses: bsvAddressPool,
        )

        let pushNotificationEnablerService = PushNotificationEnablerService(preferencesService: preferencesService)
        let notificationPermissions = BsvHomeCoreNotificationPermissions(service: pushNotificationEnablerService)
        let bannerService = BsvHomeCore.BsvHomeBannerService(store: bsvHomeCoreBannerStore)
        let navigationPresenter = NavigationPresenter(assetsService: assetsService, nftService: nftService, recentActivity: recentAssetsService)
        let bsvHomeCorePerpetualStore = BsvHomeCorePerpetualStore(store: storeManager.perpetualStore, balanceStore: storeManager.balanceStore)
        let perpetualService = gatewayService.perpetualService(
            price: priceService,
            store: bsvHomeCorePerpetualStore,
            assetStore: bsvHomeCoreAssetStore,
            preferences: preferencesService,
            balance: balanceService,
            walletPreferences: walletPreferencesService,
            session: walletSessionService,
        )
        let portfolioService = BsvHomeCore.BsvHomePortfolioService.newWithEnvironment(
            api: deviceApiClient,
            store: BsvHomeCorePortfolioStore(assetStore: storeManager.assetStore),
            price: priceService,
            perpetual: perpetualService,
            preferences: preferencesService,
            environment: .current,
        )
        let priceAlertService = BsvHomeCore.BsvHomePriceAlertService.newWithEnvironment(
            api: deviceApiClient,
            preferences: preferencesService,
            store: bsvHomeCorePriceAlertStore,
            device: deviceService,
            permissions: notificationPermissions,
            environment: .current,
        )
        let fiatService = BsvHomeCore.BsvHomeFiatService(
            api: deviceApiClient,
            assets: assetsService,
            store: BsvHomeCoreFiatStore(store: storeManager.fiatTransactionStore),
        )
        let bsvHomeCoreSupportStore = BsvHomeCoreSupportStore(store: storeManager.supportChatStore)
        let supportService = BsvHomeCore.BsvHomeSupportService(api: deviceApiClient, store: bsvHomeCoreSupportStore, files: bsvHomeCoreFileStore, provider: nativeProvider)
        let streamService = BsvHomeCore.BsvHomeStreamService(
            price: priceService,
            priceAlert: priceAlertService,
            balance: balanceService,
            transactions: transactionsService,
            nft: nftService,
            perpetual: perpetualService,
            fiat: fiatService,
            notifications: bsvHomeCoreNotificationStore,
            support: bsvHomeCoreSupportStore,
            subscriptions: streamSubscriptionService,
            session: walletSessionService,
            device: deviceService,
        )
        let streamObserverService = StreamObserverService(
            service: streamService,
            webSocket: webSocket,
        )
        let swapper = BsvHomeSwapper(rpcProvider: NativeProvider(), preferences: preferencesStore)
        let swapService = BsvHomeCore.BsvHomeSwapService.newWithEnvironment(
            swapper: swapper,
            keystore: storages.keystore.bsvHomeKeystore,
            password: BsvHomeCoreKeystorePassword(keystore: storages.keystore),
            store: BsvHomeCoreSwapStore(
                assetStore: storeManager.assetStore,
                transactionStore: storeManager.transactionStore,
                recentActivityStore: storeManager.recentActivityStore,
            ),
            environment: .current,
        )

        let chainService = BsvHomeCore.BsvHomeChainService.newWithEnvironment(environment: .current)
        let addressService = BsvHomeCore.BsvHomeAddressService()
        let walletConnectorPresenter = WalletConnectorPresenter()
        let walletConnectorInteractor = WalletConnectorInteractor(presenter: walletConnectorPresenter)
        let walletConnector = Self.makeWalletConnector(
            connectionsStore: storeManager.connectionsStore,
            interactor: walletConnectorInteractor,
            transactionSimulationService: transactionSimulationService,
            walletSessionService: walletSessionService,
            assetsService: assetsService,
            chainService: chainService,
        )

        let assetDiscoveryService = BsvHomeCore.BsvHomeAssetDiscoveryService(
            api: deviceApiClient,
            balance: balanceService,
            transactions: transactionsService,
            nft: nftService,
            walletStore: bsvHomeCoreWalletStore,
            preferences: walletPreferencesService,
        )

        let configService = BsvHomeCore.BsvHomeConfigService(api: apiClient, preferences: preferencesService)
        let appUpdateService = BsvHomeCore.BsvHomeAppUpdateService(config: configService, preferences: preferencesService)
        let rateService = RateService(preferencesService: preferencesService)

        let appStartService = BsvHomeCore.BsvHomeAppStartService(
            config: configService,
            banners: bannerService,
            assets: assetsService,
            balance: balanceService,
            walletConfiguration: BsvHomeCore.BsvHomeWalletConfigurationService(
                api: deviceApiClient,
                banners: bsvHomeCoreBannerStore,
                preferences: walletPreferencesService,
                session: walletSessionService,
            ),
            wallet: walletService,
            device: deviceService,
        )

        let onStartService = OnstartService(
            appStartService: appStartService,
            preferencesService: preferencesService,
            keystore: storages.keystore,
            session: walletSessionService,
        )

        let hyperliquidWebSocket = WebSocketConnection(
            url: nodeService.webSocketNode(for: .hyperCore),
            reconnection: connectionService,
        )
        let hyperliquidObserverService = HyperliquidObserverService(
            webSocket: hyperliquidWebSocket,
            perpetualService: perpetualService,
            streamService: BsvHomeCore.BsvHomePerpetualStreamService(
                perpetual: perpetualService,
                connection: PerpetualStreamConnection(webSocket: hyperliquidWebSocket),
            ),
        )

        let nameService = BsvHomeCore.BsvHomeNameService(api: deviceApiClient, store: bsvHomeCoreAddressStore)
        let rewardsService = BsvHomeCore.BsvHomeRewardsService(
            api: deviceApiClient,
            auth: BsvHomeCore.BsvHomeAuthService(
                api: deviceApiClient,
                keystore: storages.keystore.bsvHomeKeystore,
                password: BsvHomeCoreKeystorePassword(keystore: storages.keystore),
                deviceKey: deviceKeyService,
            ),
            balance: balanceService,
        )
        let toastPresenter = ToastPresenter()
        let pushNotificationService = BsvHomeCore.BsvHomePushNotificationService()
        let navigationHandler = NavigationHandler(
            navigationState: navigation,
            presenter: navigationPresenter,
            assetsService: assetsService,
            assetStore: storeManager.assetStore,
            walletConnector: walletConnector,
            toastPresenter: toastPresenter,
            pushNotificationService: pushNotificationService,
            transactionStore: storeManager.transactionStore,
            deeplinkService: BsvHomeCore.BsvHomeDeeplinkService(),
            paymentService: paymentService,
            transactionStateService: transactionStateService,
            walletConnectorPresenter: walletConnectorPresenter,
            walletSessionService: walletSessionService,
        )
        let searchService = BsvHomeCore.BsvHomeSearchService(
            assets: assetsService,
            assetStore: bsvHomeCoreAssetStore,
            price: priceService,
            perpetualStore: bsvHomeCorePerpetualStore,
            store: BsvHomeCoreSearchStore(store: storeManager.searchStore, assetListStore: storeManager.assetListStore),
        )
        let inAppNotificationService = BsvHomeCore.BsvHomeNotificationService(
            api: deviceApiClient,
            store: bsvHomeCoreNotificationStore,
            preferences: walletPreferencesService,
            session: walletSessionService,
        )

        let contactService = BsvHomeCore.BsvHomeContactService.newWithEnvironment(
            store: BsvHomeCoreContactStore(store: storeManager.contactStore),
            addressStore: bsvHomeCoreAddressStore,
            files: bsvHomeCoreFileStore,
            environment: .current,
        )

        let bsvAddressPoolStore = storeManager.bsvAddressPoolStore
        let bsvReceiptObserver = BsvReceiptObserverService(
            makeService: {
                try gatewayService.bsvReceiptService(
                    addresses: bsvAddressPool,
                    session: walletSessionService,
                    balances: balanceService,
                    transactions: transactionsService,
                    transactionState: transactionStateService,
                    transactionStatus: BsvHomeCoreTransactionStatusService(service: transactionStateService),
                    baseURL: portalBaseURL,
                )
            },
            makeWebSocket: { url in
                guard let url = URL(string: url) else { throw AnyError("Invalid BSV receipt WebSocket URL") }
                let configuration = URLSessionConfiguration.ephemeral
                configuration.httpCookieStorage = nil
                configuration.httpShouldSetCookies = false
                configuration.urlCredentialStorage = nil
                configuration.urlCache = nil
                return WebSocketConnection(configuration: WebSocketConfiguration(
                    url: url,
                    reconnection: connectionService,
                    sessionConfiguration: configuration,
                    redirectPolicy: .reject,
                    receiveBufferLimit: 32,
                    maximumMessageSize: 150_000,
                    queueMessagesWhileConnecting: false,
                ))
            },
            reconnection: connectionService,
            addressChanges: { bsvAddressPoolStore.observeChanges() },
        )
        let appLifecycleService = AppLifecycleService(
            walletConnector: walletConnector,
            connectionStatusObserver: connectionStatusObserver,
            deviceService: deviceService,
            subscriptionsObserver: storeManager.walletStore.observer(),
            streamObserverService: streamObserverService,
            perpetualService: perpetualService,
            perpetualObserver: hyperliquidObserverService,
            walletSessionService: walletSessionService,
            transactionStateService: transactionStateService,
            bsvReceiptObserver: bsvReceiptObserver,
            environment: .current,
        )

        let portalKeystore = storages.keystore
        let bsvColdWalletStore = portalKeystore.bsvColdWalletStore
        let confirmService = gatewayService.confirmService(
            simulation: transactionSimulationService,
            scanner: scanService,
            transactionState: transactionStateService,
            balance: balanceService,
            price: priceService,
            assets: assetsService,
            transactionStatus: BsvHomeCoreTransactionStatusService(service: transactionStateService),
            addresses: bsvAddressPool,
            coldWalletStore: bsvColdWalletStore,
        )
        let platformOrigin = Bundle.main.object(forInfoDictionaryKey: "WowPayPlatformOrigin") as? String ?? ""
        let trustedPaymailOrigins = Bundle.main.object(forInfoDictionaryKey: "WowPayPaymailTrustedOrigins") as? [String] ?? []
        let makePortalAuth: @Sendable () throws -> BsvHomePortalAuthService = {
            let language = Bundle.main.preferredLocalizations.first ?? "en"
            return try BsvHomePortalAuthService(
                provider: NativeProvider.platform(baseURL: portalBaseURL),
                keystore: portalKeystore.bsvHomeKeystore,
                password: BsvHomeCoreKeystorePassword(keystore: portalKeystore),
                session: walletSessionService,
                deviceKey: deviceKeyService,
                store: BsvHomeCoreSecurePreferencesStore(namespace: "portal_bsvhome", accessibility: .whenUnlockedThisDeviceOnly),
                baseUrl: portalBaseURL,
                language: language.hasPrefix("zh") ? "zh-CN" : language.hasPrefix("th") ? "th" : "en",
                deviceType: "4",
            )
        }

        let makeFixedPortalAuth: @Sendable (Primitives.Wallet) throws -> BsvHomePortalAuthService = { wallet in
            let language = Bundle.main.preferredLocalizations.first ?? "en"
            return try BsvHomePortalAuthService.newForWallet(
                provider: NativeProvider.platform(baseURL: portalBaseURL),
                keystore: portalKeystore.bsvHomeKeystore,
                password: BsvHomeCoreKeystorePassword(keystore: portalKeystore),
                session: walletSessionService,
                deviceKey: deviceKeyService,
                store: BsvHomeCoreSecurePreferencesStore(namespace: "portal_bsvhome_multisig_\(wallet.id.id)", accessibility: .whenUnlockedThisDeviceOnly),
                baseUrl: portalBaseURL,
                language: language.hasPrefix("zh") ? "zh-CN" : language.hasPrefix("th") ? "th" : "en",
                deviceType: "4",
                wallet: wallet.map(),
            )
        }

        let amountService = BsvHomeCore.BsvHomeAmountService(stake: stakeService, preferences: preferencesService, session: walletSessionService)
        let paymailDestinationService = BsvHomePaymailDestinationService(provider: NativeProvider.paymail())
        let phaseOneFactory = MainActor.assumeIsolated {
            PhaseOneServicesFactory {
                let baseURL = platformOrigin + "/v2/api/"
                let installationKey = "WowPayPlatformInstallation"
                let installationId: String
                if let stored = UserDefaults.standard.string(forKey: installationKey) {
                    installationId = stored
                } else {
                    installationId = UUID().uuidString
                    UserDefaults.standard.set(installationId, forKey: installationKey)
                }
                let keychain = KeychainDefault()
                let account = try BsvHomePlatformAccountService(
                    provider: NativeProvider.platform(baseURL: baseURL),
                    store: BsvHomeCoreSecurePreferencesStore(namespace: "platform", keychain: keychain, accessibility: .whenUnlockedThisDeviceOnly),
                    authenticator: BsvHomeCorePlatformAuthenticator(),
                    baseUrl: baseURL,
                    language: Bundle.main.preferredLocalizations.first?.hasPrefix("zh") == true ? "zh-CN" : "en",
                    deviceId: installationId,
                )
                let device = BsvHomeCoreMpcDeviceProof(namespace: baseURL, installationId: installationId)
                let mpc = BsvHomeMpcWalletService(
                    account: account,
                    participant: BsvHomeCoreMpcParticipantBridge(account: account, environment: baseURL, keychain: keychain),
                    device: device,
                    store: BsvHomeCoreSecurePreferencesStore(namespace: "mpc", keychain: keychain, accessibility: .whenUnlockedThisDeviceOnly),
                    paymail: paymailDestinationService,
                )
                return PhaseOneServices(
                    account: account,
                    mpc: mpc,
                    hd: BsvHomePlatformHdService(platform: account, session: walletSessionService, addresses: bsvAddressPool, keystore: portalKeystore.bsvHomeKeystore, password: BsvHomeCoreKeystorePassword(keystore: portalKeystore)),
                    alternative: BsvHomePlatformAlternativeService(account: account, device: device),
                    recipientInputs: MpcRecipientInputCoordinator(
                        service: BsvHomeSendService(
                            recipient: BsvHomeRecipientService(names: nameService, payments: paymentService, session: walletSessionService),
                            amount: amountService,
                            paymail: paymailDestinationService,
                        ),
                    ),
                )
            }
        }

        let viewModelFactory = ViewModelFactory(
            apiClient: apiClient,
            assetConfig: BsvHomeCore.BsvHomeAssetConfigService(),
            assetDiscoveryService: assetDiscoveryService,
            assetsService: assetsService,
            avatarService: avatarService,
            bannerService: bannerService,
            balanceService: balanceService,
            bsvAddressPool: bsvAddressPool,
            confirmService: confirmService,
            contactService: contactService,
            manageContactService: BsvHomeCore.BsvHomeManageContactService(
                contacts: contactService,
                addresses: addressService,
                payments: paymentService,
            ),
            deeplinkService: BsvHomeCore.BsvHomeDeeplinkService(),
            explorerService: explorerService,
            fiatService: fiatService,
            gatewayService: gatewayService,
            hyperliquidObserverService: hyperliquidObserverService,
            nameService: nameService,
            nftService: nftService,
            nodeService: nodeService,
            paymentService: paymentService,
            perpetualService: perpetualService,
            portfolioService: portfolioService,
            preferencesService: preferencesService,
            priceAlertService: priceAlertService,
            priceService: priceService,
            rewardsService: rewardsService,
            searchService: searchService,
            simulationFormatter: BsvHomeCore.BsvHomeSimulationFormatter(),
            stakeService: stakeService,
            streamSubscriptionService: streamSubscriptionService,
            swapService: swapService,
            transactionStateService: transactionStateService,
            transactionsService: transactionsService,
            walletService: walletService,
            walletSessionService: walletSessionService,
            serviceStatusService: serviceStatusService,
            appUpdateService: appUpdateService,
            appStartService: appStartService,
            inAppNotificationService: inAppNotificationService,
            biometryService: BiometryAuthenticationService(
                keystorePassword: LocalKeystorePassword(),
                securityService: BsvHomeCore.BsvHomeSecurityService(),
            ),
            keystore: storages.keystore,
            bsvHomeCoreKeystore: storages.keystore.bsvHomeKeystore,
            observablePreferences: observablePreferences,
            recentAssetsService: recentAssetsService,
            amountService: amountService,
            toastPresenter: toastPresenter,
            walletPreferencesService: walletPreferencesService,
            signMessageService: BsvHomeCore.BsvHomeSignMessageService(
                names: nameService,
                explorer: explorerService,
                keystore: storages.keystore.bsvHomeKeystore,
                password: BsvHomeCoreKeystorePassword(keystore: storages.keystore),
            ),
            developerService: BsvHomeCore.BsvHomeDeveloperService(
                platform: devicePlatform,
                preferences: preferencesService,
                walletPreferences: walletPreferencesService,
                transactions: transactionStateStore,
                perpetual: perpetualService,
            ),
            deviceService: deviceService,
            notificationPermissions: notificationPermissions,
            storeManager: storeManager,
            supportService: supportService,
            supportTyping: bsvHomeCoreSupportStore.typing,
            paymailDestinationService: paymailDestinationService,
            phaseOneServices: phaseOneFactory,
            makePaymailService: { walletId in
                try BsvHomePaymailService(auth: makePortalAuth(), trustedPublicOrigins: trustedPaymailOrigins, walletId: walletId)
            },
            makeWalletPaymailDestinationService: { walletId in
                let sender = try BsvHomePaymailService(auth: makePortalAuth(), trustedPublicOrigins: trustedPaymailOrigins, walletId: walletId)
                return BsvHomePaymailDestinationService.newForWallet(provider: NativeProvider.paymail(), sender: sender)
            },
            makeTransferPasswordService: { walletId in
                try BsvHomePortalTransferPasswordService(auth: makePortalAuth(), walletId: walletId)
            },
            makeFixedTransferPasswordService: { wallet in
                try BsvHomePortalTransferPasswordService.newForWallet(auth: makeFixedPortalAuth(wallet), wallet: wallet.map())
            },
            makePortalSwapService: {
                let auth = try makePortalAuth()
                return gatewayService.portalSwapService(
                    auth: auth,
                    store: BsvHomeCorePortalSwapStore(store: storeManager.portalSwapStore),
                    paymentStore: BsvHomeCoreSecurePreferencesStore(namespace: "portalpayments", accessibility: .whenUnlockedThisDeviceOnly),
                    assets: assetsService,
                    addresses: bsvAddressPool,
                )
            },
            makeBsvMultisigService: { wallet, authorization in
                let auth = try makeFixedPortalAuth(wallet)
                return try gatewayService.bsvMultisigService(
                    store: BsvHomeCoreBsvMultisigStore(origin: portalOrigin, localWalletId: wallet.id.id),
                    session: walletSessionService,
                    signer: BsvHomeCoreBsvMultisigSigner(keystore: portalKeystore),
                    auth: auth,
                    addresses: bsvAddressPool,
                    walletId: wallet.id.id,
                    authorization: authorization,
                    coldStore: bsvColdWalletStore,
                )
            },
            makeBsvPublicFundingService: { payer, authorization in
                try gatewayService.bsvPublicFundingService(
                    session: walletSessionService,
                    addresses: bsvAddressPool,
                    signer: BsvHomeCoreBsvMultisigSigner(keystore: portalKeystore),
                    authorization: authorization,
                    payerAuth: makeFixedPortalAuth(payer),
                    payer: payer.map(),
                    preferences: preferencesService,
                    coldStore: bsvColdWalletStore,
                )
            },
            publicFundingPayers: { wallets, entry in
                let mapped = wallets.map { $0.map() }
                return switch entry {
                case let .payment(source):
                    try await bsvMultisigPublicFundingPayers(wallets: mapped, preferences: preferencesService, coldStore: bsvColdWalletStore, source: source)
                case .recovery:
                    try bsvMultisigFundingPayers(wallets: mapped)
                }
            },
            makeBsvColdWalletService: { authorization in
                gatewayService.bsvColdWalletService(
                    wallet: walletService,
                    addresses: bsvAddressPool,
                    keystore: portalKeystore.bsvHomeKeystore,
                    store: bsvColdWalletStore,
                    signer: BsvHomeCoreBsvColdWalletSigner(
                        keystore: portalKeystore,
                        authenticator: BiometryAuthenticationService(keystorePassword: LocalKeystorePassword(), securityService: BsvHomeCore.BsvHomeSecurityService()),
                        reason: Localized.coldWalletReview,
                        authorizeSigning: { wallet, scope in try await authorization.authorize(wallet: wallet, scope: scope) },
                    ),
                )
            },
            bsvColdWalletStore: bsvColdWalletStore,
            bsvColdWalletPasswordStore: BsvHomeCoreSecurePreferencesStore(namespace: "offline_signing_password_", accessibility: .whenUnlockedThisDeviceOnly),
            wowPayPortalBaseURL: portalBaseURL,
        )

        return AppResolver.Services(
            walletConnector: walletConnector,
            connectionStatusObserver: connectionStatusObserver,
            deviceService: deviceService,
            navigationHandler: navigationHandler,
            navigationPresenter: navigationPresenter,
            streamObserverService: streamObserverService,
            transactionStateService: transactionStateService,
            observablePreferences: observablePreferences,
            walletSessionService: walletSessionService,
            appUpdateService: appUpdateService,
            rateService: rateService,
            onstartService: onStartService,
            appStartService: appStartService,
            pushNotificationEnablerService: pushNotificationEnablerService,
            walletConnectorPresenter: walletConnectorPresenter,
            toastPresenter: toastPresenter,
            viewModelFactory: viewModelFactory,
            appLifecycleService: appLifecycleService,
        )
    }
}

// MARK: - Private Static

extension ServicesFactory {
    private static func makeDeviceApiClient(
        provider: NativeProvider,
        deviceKey: BsvHomeCore.BsvHomeDeviceKeyService,
    ) -> BsvHomeCore.BsvHomeDeviceApiClient {
        BsvHomeCore.BsvHomeDeviceApiClient(
            provider: provider,
            deviceKey: deviceKey,
        )
    }

    private static func makeWalletConnector(
        connectionsStore: ConnectionStore,
        interactor: WalletConnectorInteractor,
        transactionSimulationService: BsvHomeSimulationService,
        walletSessionService: BsvHomeWalletSessionService,
        assetsService: BsvHomeAssetsService,
        chainService: BsvHomeCore.BsvHomeChainService,
    ) -> WalletConnectorService {
        WalletConnectorService(
            walletSessionService: walletSessionService,
            interactor: interactor,
            service: BsvHomeWalletConnectService(
                simulation: transactionSimulationService,
                store: BsvHomeCoreConnectionStore(store: connectionsStore),
                signer: interactor,
                session: walletSessionService,
                assets: assetsService,
            ),
            chainService: chainService,
        )
    }

    private static func makeWebSocket(deviceKeyService: BsvHomeDeviceKeyService, reconnection: any Reconnectable) -> any WebSocketConnectable {
        let requestProvider = AuthenticatedRequestProvider(deviceKeyService: deviceKeyService)
        let configuration = WebSocketConfiguration(requestProvider: requestProvider, reconnection: reconnection)
        return WebSocketConnection(configuration: configuration)
    }
}
