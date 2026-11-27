// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import BsvHomeCore
import NativeProviderService
import Primitives

public actor GatewayService: Sendable {
    let gateway: BsvHomeGateway
    private let preferences: any BsvHomePreferencesStore
    private let securePreferences: any BsvHomeSecureStore
    private let environment: BsvHomeRuntimeEnvironment

    public init(
        provider: NativeProvider,
        preferences: any BsvHomePreferencesStore,
        securePreferences: any BsvHomeSecureStore,
        environment: BsvHomeRuntimeEnvironment = .production,
    ) {
        self.preferences = preferences
        self.securePreferences = securePreferences
        self.environment = environment
        gateway = BsvHomeGateway.newWithEnvironment(
            provider: provider,
            preferences: preferences,
            securePreferences: securePreferences,
            environment: environment,
        )
    }

    public nonisolated func with(provider: NativeProvider) -> GatewayService {
        GatewayService(provider: provider, preferences: preferences, securePreferences: securePreferences, environment: environment)
    }

    public nonisolated func bsvAddressPool(
        walletStore: any BsvHomeWalletStore,
        store: any BsvHomeBsvAddressStore,
        keystore: BsvHomeKeystore,
        password: any BsvHomeKeystorePassword,
    ) -> BsvHomeBsvAddressPool {
        BsvHomeBsvAddressPool(gateway: gateway, walletStore: walletStore, store: store, keystore: keystore, password: password)
    }

    public nonisolated func bsvMultisigService(store: any BsvHomeBsvMultisigStore, session: BsvHomeWalletSessionService, signer: any BsvHomeBsvMultisigSigner, auth: BsvHomePortalAuthService, addresses: BsvHomeBsvAddressPool, walletId: String, authorization: any BsvHomeBsvMultisigAuthorization, coldStore: any BsvHomeBsvColdWalletStore) -> BsvHomeBsvMultisigService {
        BsvHomeBsvMultisigService.newWithAuthorization(store: store, session: session, gateway: gateway, signer: signer, auth: auth, addresses: addresses, walletId: walletId, authorization: authorization, coldStore: coldStore)
    }

    public nonisolated func bsvPublicFundingService(
        session: BsvHomeWalletSessionService,
        addresses: BsvHomeBsvAddressPool,
        signer: any BsvHomeBsvMultisigSigner,
        authorization: any BsvHomeBsvMultisigAuthorization,
        payerAuth: BsvHomePortalAuthService,
        payer: BsvHomeCore.Wallet,
        preferences: BsvHomePreferencesService,
        coldStore: any BsvHomeBsvColdWalletStore,
    ) throws -> BsvHomeBsvMultisigPublicFundingService {
        try BsvHomeBsvMultisigPublicFundingService(session: session, gateway: gateway, addresses: addresses, signer: signer, authorization: authorization, payerAuth: payerAuth, payer: payer, preferences: preferences, coldStore: coldStore)
    }

    public nonisolated func bsvReceiptService(
        addresses: BsvHomeBsvAddressPool,
        session: BsvHomeWalletSessionService,
        balances: BsvHomeBalanceService,
        transactions: BsvHomeTransactionsService,
        transactionState: BsvHomeTransactionStateService,
        transactionStatus: any BsvHomeTransactionStatusService,
        baseURL: String,
    ) throws -> BsvHomeBsvReceiptService {
        try BsvHomeBsvReceiptService(gateway: gateway, addresses: addresses, session: session, balances: balances, transactions: transactions, transactionState: transactionState, transactionStatus: transactionStatus, baseUrl: baseURL)
    }

    public nonisolated func bsvColdWalletService(wallet: BsvHomeWalletService, addresses: BsvHomeBsvAddressPool, keystore: BsvHomeKeystore, store: any BsvHomeBsvColdWalletStore, signer: any BsvHomeBsvColdWalletSigner) -> BsvHomeBsvColdWalletService {
        BsvHomeBsvColdWalletService.unified(wallet: wallet, addresses: addresses, gateway: gateway, keystore: keystore, store: store, signer: signer)
    }

    public nonisolated func transactionsService(
        api: BsvHomeDeviceApiClient,
        assets: BsvHomeAssetsService,
        store: any BsvHomeTransactionStore,
        addressStore: any BsvHomeAddressStore,
        walletPreferences: BsvHomeWalletPreferencesService,
        preferences: BsvHomePreferencesService,
        session: BsvHomeWalletSessionService,
        transactionStatus: any BsvHomeTransactionStatusService,
        addresses: BsvHomeBsvAddressPool,
    ) -> BsvHomeTransactionsService {
        BsvHomeTransactionsService(api: api, gateway: gateway, assets: assets, store: store, addressStore: addressStore, walletPreferences: walletPreferences, preferences: preferences, session: session, transactionStatus: transactionStatus, addresses: addresses)
    }

    public nonisolated func chainSettingsService(nodes: BsvHomeNodeService, explorer: BsvHomeExplorerService) -> BsvHomeChainSettingsService {
        BsvHomeChainSettingsService(nodes: nodes, explorer: explorer, gateway: gateway)
    }

    public nonisolated func stakeService(
        staticApi: BsvHomeStaticApiClient,
        store: any BsvHomeStakeStore,
        addressStore: any BsvHomeAddressStore,
        explorer: BsvHomeExplorerService,
        preferences: BsvHomePreferencesService,
        session: BsvHomeWalletSessionService,
    ) -> BsvHomeStakeService {
        BsvHomeStakeService(gateway: gateway, staticApi: staticApi, store: store, addressStore: addressStore, explorer: explorer, preferences: preferences, session: session)
    }

    public nonisolated func transactionStateService(
        store: any BsvHomeTransactionStateStore,
        assets: BsvHomeAssetsService,
        balance: BsvHomeBalanceService,
        stake: BsvHomeStakeService,
        nft: BsvHomeNftService,
    ) -> BsvHomeTransactionStateService {
        BsvHomeTransactionStateService(gateway: gateway, store: store, assets: assets, balance: balance, stake: stake, nft: nft)
    }

    public nonisolated func balanceService(
        walletStore: any BsvHomeWalletStore,
        assetStore: any BsvHomeAssetStore,
        store: any BsvHomeBalanceStore,
        assets: BsvHomeAssetsService,
        stream: BsvHomeStreamSubscriptionService,
        addresses: BsvHomeBsvAddressPool,
    ) -> BsvHomeBalanceService {
        BsvHomeBalanceService(gateway: gateway, walletStore: walletStore, assetStore: assetStore, store: store, assets: assets, stream: stream, addresses: addresses)
    }

    public nonisolated func portalSwapService(
        auth: BsvHomePortalAuthService,
        store: any BsvHomePortalSwapStore,
        paymentStore: any BsvHomeSecureStore,
        assets: BsvHomeAssetsService,
        addresses: BsvHomeBsvAddressPool,
    ) -> BsvHomePortalSwapService {
        BsvHomePortalSwapService(auth: auth, store: store, paymentStore: paymentStore, assets: assets, gateway: gateway, addresses: addresses)
    }

    public nonisolated func assetsService(
        api: BsvHomeApiClient,
        store: any BsvHomeAssetStore,
        price: BsvHomePriceService,
        preferences: BsvHomePreferencesService,
        session: BsvHomeWalletSessionService,
    ) -> BsvHomeAssetsService {
        BsvHomeAssetsService(api: api, gateway: gateway, store: store, price: price, preferences: preferences, session: session)
    }

    public nonisolated func perpetualService(
        price: BsvHomePriceService,
        store: any BsvHomePerpetualStore,
        assetStore: any BsvHomeAssetStore,
        preferences: BsvHomePreferencesService,
        balance: BsvHomeBalanceService,
        walletPreferences: BsvHomeWalletPreferencesService,
        session: BsvHomeWalletSessionService,
    ) -> BsvHomePerpetualService {
        BsvHomePerpetualService(
            gateway: gateway,
            price: price,
            store: store,
            assetStore: assetStore,
            preferences: preferences,
            balance: balance,
            walletPreferences: walletPreferences,
            session: session,
        )
    }

    public nonisolated func confirmService(
        simulation: BsvHomeSimulationService,
        scanner: BsvHomeScanService,
        transactionState: BsvHomeTransactionStateService,
        balance: BsvHomeBalanceService,
        price: BsvHomePriceService,
        assets: BsvHomeAssetsService,
        transactionStatus: any BsvHomeTransactionStatusService,
        addresses: BsvHomeBsvAddressPool,
        coldWalletStore: any BsvHomeBsvColdWalletStore,
    ) -> BsvHomeConfirmService {
        BsvHomeConfirmService(gateway: gateway, simulation: simulation, scanner: scanner, transactionState: transactionState, balance: balance, price: price, assets: assets, transactionStatus: transactionStatus, addresses: addresses, coldWalletStore: coldWalletStore)
    }
}
