// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import func BsvHomeCore.bsvMultisigScanDestination
import enum BsvHomeCore.Deeplink
import protocol BsvHomeCore.BsvHomeAssetsServiceProtocol
import class BsvHomeCore.BsvHomeDeeplinkService
import class BsvHomeCore.BsvHomePaymentService
import enum BsvHomeCore.BsvHomePushNotification
import protocol BsvHomeCore.BsvHomePushNotificationServiceProtocol
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import protocol BsvHomeCore.BsvHomeTransactionStateServiceProtocol
import protocol BsvHomeCore.BsvHomeWalletSessionServiceProtocol
import enum BsvHomeCore.Payment
import enum BsvHomeCore.UrlAction
import enum BsvHomeCore.WalletConnectLink
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI
import WalletConnector
import WalletConnectorService

@Observable
final class NavigationHandler: Sendable {
    private let navigationState: NavigationStateManager
    private let presenter: NavigationPresenter

    private let assetsService: any BsvHomeAssetsServiceProtocol
    private let assetStore: AssetStore
    private let walletConnector: any WalletConnectorServiceable
    private let toastPresenter: ToastPresenter
    private let pushNotificationService: any BsvHomePushNotificationServiceProtocol
    private let transactionStore: TransactionStore
    private let deeplinkService: BsvHomeDeeplinkService
    private let paymentService: BsvHomePaymentService
    private let transactionStateService: any BsvHomeTransactionStateServiceProtocol
    private let walletConnectorPresenter: WalletConnectorPresenter
    private let walletSessionService: any BsvHomeWalletSessionServiceProtocol

    init(
        navigationState: NavigationStateManager,
        presenter: NavigationPresenter,
        assetsService: any BsvHomeAssetsServiceProtocol,
        assetStore: AssetStore,
        walletConnector: any WalletConnectorServiceable,
        toastPresenter: ToastPresenter,
        pushNotificationService: any BsvHomePushNotificationServiceProtocol,
        transactionStore: TransactionStore,
        deeplinkService: BsvHomeDeeplinkService,
        paymentService: BsvHomePaymentService,
        transactionStateService: any BsvHomeTransactionStateServiceProtocol,
        walletConnectorPresenter: WalletConnectorPresenter,
        walletSessionService: any BsvHomeWalletSessionServiceProtocol,
    ) {
        self.navigationState = navigationState
        self.presenter = presenter
        self.assetsService = assetsService
        self.assetStore = assetStore
        self.walletConnector = walletConnector
        self.toastPresenter = toastPresenter
        self.pushNotificationService = pushNotificationService
        self.transactionStore = transactionStore
        self.deeplinkService = deeplinkService
        self.paymentService = paymentService
        self.transactionStateService = transactionStateService
        self.walletConnectorPresenter = walletConnectorPresenter
        self.walletSessionService = walletSessionService
    }

    @MainActor
    func handlePush(_ userInfo: [AnyHashable: Any]) async {
        guard
            let notificationType = userInfo["type"] as? String,
            let notification = pushNotificationService.parse(
                notificationType: notificationType,
                data: Self.payload(userInfo["data"]),
            )
        else {
            return
        }
        do {
            try await handle(notification)
        } catch {
            debugLog("NavigationHandler push error: \(error)")
        }
    }

    private static func payload(_ data: Any?) -> String? {
        guard let data, JSONSerialization.isValidJSONObject(data) else { return .none }
        return (try? JSONSerialization.data(withJSONObject: data)).map { String(decoding: $0, as: UTF8.self) }
    }

    @MainActor
    func handle(url: URL) async {
        guard !routeMultisig(code: url.absoluteString) else { return }
        await handleStandardCode(url.absoluteString)
    }

    @MainActor
    func handle(code: String) async {
        await handle(code: code, isActive: { true })
    }

    @MainActor
    func handle(code: String, isActive: @escaping @MainActor () -> Bool) async {
        guard isActive(), !routeMultisigScan(code: code, isActive: isActive) else { return }
        await handleStandardCode(code, isActive: isActive)
    }

    @MainActor
    func routeMultisigScan(code: String, isActive: @escaping @MainActor () -> Bool) -> Bool {
        guard isActive() else { return true }
        if routeMultisig(code: code, isActive: isActive) {
            return true
        }
        if case let .collective(signerWalletId, walletId) = navigationState.walletPresentation {
            presenter.requestMultisig(.collectiveSend(signerWalletId: signerWalletId.id, walletId: walletId, code: code), isActive: isActive)
            return true
        }
        return false
    }

    @MainActor
    private func routeMultisig(code: String, isActive: @escaping @MainActor () -> Bool = { true }) -> Bool {
        do {
            guard let destination = try bsvMultisigScanDestination(value: code) else { return false }
            switch destination {
            case let .payment(request): presenter.requestMultisig(.publicFunding(.paymentRequest(request: request)), isActive: isActive)
            case let .invitation(token): presenter.requestMultisig(.invitation(token), isActive: isActive)
            }
        } catch {
            showError(AnyError(Localized.Errors.notSupported))
        }
        return true
    }

    @MainActor
    private func handleStandardCode(_ code: String, isActive: @escaping @MainActor () -> Bool = { true }) async {
        guard let action = deeplinkService.urlAction(url: code) else {
            return showError(AnyError(Localized.Errors.notSupported))
        }
        await handle(action, isActive: isActive)
    }

    @MainActor
    func handle(_ action: UrlAction) async {
        await handle(action, isActive: { true })
    }

    @MainActor
    private func handle(_ action: UrlAction, isActive: @escaping @MainActor () -> Bool) async {
        do {
            guard isActive() else { return }
            try await handleURLAction(action, isActive: isActive)
        } catch {
            guard isActive() else { return }
            toastPresenter.toastMessage = nil
            showError(error)
        }
    }

    @MainActor
    func open(url: URL) -> Bool {
        if routeMultisig(code: url.absoluteString) {
            return true
        }
        guard let action = deeplinkService.urlAction(url: url.absoluteString) else { return false }
        Task { await handle(action) }
        return true
    }
}

// MARK: - UrlAction

@MainActor
extension NavigationHandler {
    private func handleURLAction(_ action: UrlAction, isActive: @escaping @MainActor () -> Bool) async throws {
        switch action {
        case let .deeplink(deeplink): try await handleDeepLink(deeplink)
        case let .payment(payment): try await handlePayment(payment, isActive: isActive)
        case let .walletConnect(link): await handleWalletConnect(link)
        }
    }

    private func handleDeepLink(_ deeplink: Deeplink) async throws {
        switch deeplink {
        case let .asset(assetId):
            try await navigateToAsset(AssetId(id: assetId))

        case .perpetuals:
            guard BsvHomeRuntimeEnvironment.current.allowedAssetIds() == nil else { return }
            navigationState.wallet.append(Scenes.Perpetuals())

        case let .rewards(code):
            navigationState.settings.append(Scenes.Referral(code: code))

        case let .receive(assetId):
            try await presentReceive(assetId: AssetId(id: assetId))

        case let .buy(assetId, amount):
            try await presentFiat(type: .buy, assetId: AssetId(id: assetId), amount: amount.map(\.asInt))

        case let .sell(assetId, amount):
            try await presentFiat(type: .sell, assetId: AssetId(id: assetId), amount: amount.map(\.asInt))

        case let .swap(assetId):
            try await presentSwap(from: AssetId(id: assetId), to: .none)
        }

        selectTab(for: deeplink.selectTab)
    }
}

// MARK: - Payment

@MainActor
extension NavigationHandler {
    private func handlePayment(_ payment: BsvHomeCore.Payment, isActive: @escaping @MainActor () -> Bool) async throws {
        guard let wallet = await walletSessionService.currentWallet, isActive() else { return }
        switch payment {
        case let .request(request):
            let assets = try assetStore.getAssetsData(walletId: wallet.id, filters: [], environment: .current)
            presenter.isPresentingPayment.wrappedValue = try PaymentDestinationBuilder.build(payment: request, assets: assets, paymentService: paymentService)
        case let .link(link):
            toastPresenter.toastMessage = ToastMessage(title: Localized.Common.loading, image: SystemImage.network)
            let addresses = wallet.accounts.map { ChainAddress(chain: $0.chain, address: $0.address).map() }
            let transaction = try await paymentService.load(link: link, addresses: addresses)
            guard isActive() else { return }
            let asset = try await assetsService.ensureTokenAsset(for: Primitives.AssetId(core: paymentService.transactionAssetId(transaction: transaction)))
            guard isActive() else { return }
            toastPresenter.toastMessage = nil
            presenter.isPresentingPayment.wrappedValue = PaymentDestinationBuilder.build(transaction: transaction, asset: asset, paymentService: paymentService)
        }
    }
}

// MARK: - WalletConnect

@MainActor
extension NavigationHandler {
    private func handleWalletConnect(_ link: WalletConnectLink) async {
        walletConnectorPresenter.isPresentingConnectionBar = true

        do {
            switch link {
            case let .connect(uri):
                try await walletConnector.pair(uri: uri)
            case .request:
                break
            case .session:
                walletConnector.updateSessions()
            }
        } catch {
            debugLog("NavigationHandler walletConnect error: \(error)")
            walletConnectorPresenter.isPresentingError = error.localizedDescription
        }
    }
}

// MARK: - PushNotification

@MainActor
extension NavigationHandler {
    private func handle(_ notification: BsvHomePushNotification) async throws {
        switch notification {
        case let .asset(assetId), let .priceAlert(assetId):
            try await navigateToAsset(Primitives.AssetId(id: assetId))
        case let .fiatTransaction(walletId, assetId), let .stake(walletId, assetId):
            try await navigateToAsset(walletId: Primitives.WalletId.from(id: walletId), assetId: Primitives.AssetId(id: assetId))
        case let .transaction(walletId, assetId, transaction):
            try await navigateToTransaction(
                walletId: Primitives.WalletId.from(id: walletId),
                assetId: Primitives.AssetId(id: assetId),
                transaction: transaction.map(),
            )
        case let .buyAsset(assetId):
            try await presentFiat(type: .buy, assetId: Primitives.AssetId(id: assetId), amount: .none)
        case let .swapAsset(fromAssetId, toAssetId):
            try await presentSwap(from: Primitives.AssetId(id: fromAssetId), to: Primitives.AssetId(id: toAssetId))
        case .support:
            presenter.isPresentingSupport.wrappedValue = true
        case .rewards:
            navigationState.settings.append(Scenes.Referral(code: .none))
        case .test: break
        }

        selectTab(for: notification.selectTab)
    }
}

// MARK: - Private

@MainActor
extension NavigationHandler {
    private func showError(_ error: any Error) {
        debugLog("NavigationHandler error: \(error)")
        toastPresenter.toastMessage = .error(error.localizedDescription)
    }

    private func selectTab(for tab: TabItem?) {
        guard let tab else { return }
        navigationState.selectedTab = tab
    }

    private func navigateToAsset(_ assetId: AssetId) async throws {
        guard let asset = try await assetsService.openAsset(for: assetId) else {
            return
        }
        navigationState.openAsset(asset)
    }

    private func navigateToAsset(walletId: WalletId, assetId: AssetId) async throws {
        guard let wallet = try? await walletSessionService.getWallet(walletId: walletId),
              let asset = try await assetsService.openWalletAsset(wallet: wallet, assetId: assetId)
        else {
            return
        }

        try openWallet(walletId, path: getPath(for: asset))
    }

    private func trackNotificationTransaction(walletId: WalletId, transaction: Primitives.Transaction) {
        Task {
            do {
                try await transactionStateService.track(walletId: walletId.id, transactions: [transaction.map()])
            } catch {
                debugLog("navigation: transaction tracking failed \(error)")
            }
        }
    }

    private func navigateToTransaction(walletId: WalletId, assetId: AssetId, transaction: Primitives.Transaction) async throws {
        guard let wallet = try? await walletSessionService.getWallet(walletId: walletId),
              let asset = try await transactionStateService.addNotificationTransaction(
                  wallet: wallet.map(),
                  assetId: assetId.identifier,
                  transaction: transaction.map(),
              ).map({ $0.map() })
        else {
            return
        }
        trackNotificationTransaction(walletId: walletId, transaction: transaction)
        let transaction = try transactionStore.getTransaction(walletId: walletId, transactionId: transaction.id)

        try openWallet(walletId, path: getPath(for: asset, transaction: transaction))
    }

    private func openWallet(_ walletId: WalletId, path: [any Hashable & Codable]) throws {
        guard walletSessionService.currentWalletId != walletId else {
            return navigationState.openWallet(path: path)
        }
        try walletSessionService.setCurrent(walletId: walletId)
        navigationState.pendingWalletPath = path
    }

    private func getPath(for asset: Asset) -> [any Hashable & Codable] {
        switch asset.type {
        case .perpetual: [Scenes.Perpetual(asset)]
        default: [Scenes.Asset(asset: asset)]
        }
    }

    private func getPath(for asset: Asset, transaction: TransactionExtended) -> [any Hashable & Codable] {
        switch asset.type {
        case .perpetual: [Scenes.Perpetuals(), Scenes.Perpetual(asset), Scenes.Transaction(transaction: transaction)]
        default: [Scenes.Asset(asset: asset), Scenes.Transaction(transaction: transaction)]
        }
    }

    private func presentSwap(from fromId: AssetId, to toId: AssetId?) async throws {
        guard let wallet = await walletSessionService.currentWallet else { return }
        try await presenter.presentSwap(from: fromId, to: toId, wallet: wallet)
    }

    private func presentFiat(type: FiatQuoteType, assetId: AssetId, amount: Int?) async throws {
        let asset = try await assetsService.ensureAsset(for: assetId)
        let selectedType: SelectedAssetType = switch type {
        case .buy: .buy(asset, amount: amount)
        case .sell: .sell(asset, amount: amount)
        }
        try await presentAssetInput(type: selectedType, for: asset)
    }

    private func presentReceive(assetId: AssetId) async throws {
        let asset = try await assetsService.ensureAsset(for: assetId)
        try await presentAssetInput(type: .receive(.asset), for: asset)
    }

    private func presentAssetInput(type: SelectedAssetType, for asset: Asset) async throws {
        guard let wallet = await walletSessionService.currentWallet else { return }
        try presenter.presentAssetInput(type: type, for: asset, wallet: wallet)
    }

    func resetNavigation() {
        navigationState.reset()
    }
}

// MARK: - TabItem Selection

private extension Deeplink {
    var selectTab: TabItem? {
        switch self {
        case .asset, .perpetuals: .wallet
        case .rewards: .settings
        case .receive, .buy, .sell, .swap: nil
        }
    }
}

private extension BsvHomePushNotification {
    var selectTab: TabItem? {
        switch self {
        case .transaction, .asset, .fiatTransaction, .priceAlert, .stake: .wallet
        case .buyAsset, .swapAsset: nil
        case .support, .rewards: .settings
        case .test: nil
        }
    }
}
