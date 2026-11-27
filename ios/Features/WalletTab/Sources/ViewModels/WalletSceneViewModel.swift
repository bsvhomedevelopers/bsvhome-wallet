// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Formatters
import Foundation
import func BsvHomeCore.bsvColdWalletCanSend
import struct BsvHomeCore.FiatRate
import struct BsvHomeCore.BsvHomeBannerContent
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import protocol BsvHomeCore.BsvHomeWalletHomeServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import InfoSheet
import Localization
import NFT
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI

@Observable
@MainActor
public final class WalletSceneViewModel: Sendable, AssetActions {
    private let service: any BsvHomeWalletHomeServiceProtocol
    private let onRequestScan: (@MainActor () -> Void)?

    let observablePreferences: ObservablePreferences

    public let collectionsModel: CollectionsViewModel

    public var wallet: Wallet {
        walletQuery.value
    }

    // db queries
    public let walletQuery: ObservableQuery<WalletRequest>
    public let fiatValuesQuery: ObservableQuery<AssetFiatValuesRequest>
    public let fiatRatesQuery: ObservableQuery<FiatRatesRequest>
    public let perpetualBalanceQuery: ObservableQuery<PerpetualWalletBalanceRequest>
    public let assetsQuery: ObservableQuery<AssetsRequest>
    public let fixedAssetsQuery: ObservableQuery<AssetsRequest>
    public let developmentAssetsQuery: ObservableQuery<AssetsRequest>
    public let bsvPriceUsdQuery: ObservableQuery<PriceUsdRequest>
    public let trxPriceUsdQuery: ObservableQuery<PriceUsdRequest>
    public let usdtPriceUsdQuery: ObservableQuery<PriceUsdRequest>
    public let bannersQuery: ObservableQuery<BannersRequest>

    public var isPresentingSelectedAssetInput: Binding<SelectedAssetInput?>
    public var isPresentingScanner = false
    public var isPresentingWallets: Binding<Bool>
    public var isPresentingSheet: WalletSheetType?
    public var isPresentingSearch = false
    public var isPresentingUrl: URL?
    public var isPresentingToastMessage: ToastMessage?

    public var isLoadingAssets = false
    private(set) var refreshState: StateViewType<Void> = .loading
    private var isRefreshing = false
    private let developmentAssetIds: [AssetId]

    public init(
        service: any BsvHomeWalletHomeServiceProtocol,
        observablePreferences: ObservablePreferences,
        collectionsModel: CollectionsViewModel,
        wallet: Wallet,
        isPresentingSelectedAssetInput: Binding<SelectedAssetInput?>,
        isPresentingWallets: Binding<Bool>,
        onRequestScan: (@MainActor () -> Void)? = nil,
    ) {
        self.service = service
        self.onRequestScan = onRequestScan
        self.observablePreferences = observablePreferences
        self.collectionsModel = collectionsModel
        let availableChains = service.availableChains().map { Chain(core: $0) }
        let allowedAssetIds = BsvHomeRuntimeEnvironment.current.allowedAssetIds()
        developmentAssetIds = service.developmentAssetIds(wallet: wallet.map())
            .filter { allowedAssetIds?.contains($0) ?? true }
            .map { AssetId(core: $0) }

        walletQuery = ObservableQuery(WalletRequest(walletId: wallet.id), initialValue: wallet)
        fiatValuesQuery = ObservableQuery(
            AssetFiatValuesRequest(walletId: wallet.id, priceBasis: .usd),
            initialValue: [],
        )
        fiatRatesQuery = ObservableQuery(FiatRatesRequest(), initialValue: [])
        perpetualBalanceQuery = ObservableQuery(
            PerpetualWalletBalanceRequest(walletId: wallet.id, assetId: Chain.hyperCore.defaultAsset(type: .perpetual).id),
            initialValue: nil,
        )
        assetsQuery = ObservableQuery(AssetsRequest(walletId: wallet.id, filters: [.enabledBalance], allowedChains: availableChains), initialValue: [])
        let fixedAssetIds = WowPayAsset.visibleCases.compactMap { $0.assetId?.identifier }
        fixedAssetsQuery = ObservableQuery(
            AssetsRequest(walletId: wallet.id, filters: [.chainsOrAssets([], fixedAssetIds)], limit: fixedAssetIds.count, allowedChains: availableChains),
            initialValue: [],
        )
        developmentAssetsQuery = ObservableQuery(
            AssetsRequest(walletId: wallet.id, filters: [.chainsOrAssets([], developmentAssetIds.map(\.identifier))], limit: developmentAssetIds.count, allowedChains: availableChains),
            initialValue: [],
        )
        bsvPriceUsdQuery = ObservableQuery(PriceUsdRequest(assetId: WowPayAsset.bsvAssetId), initialValue: nil)
        trxPriceUsdQuery = ObservableQuery(PriceUsdRequest(assetId: WowPayAsset.trxAssetId), initialValue: nil)
        usdtPriceUsdQuery = ObservableQuery(PriceUsdRequest(assetId: WowPayAsset.usdtAssetId), initialValue: nil)
        bannersQuery = ObservableQuery(BannersRequest(walletId: wallet.id, assetId: .none, events: [.accountBlockedMultiSignature]), initialValue: [])
        self.isPresentingSelectedAssetInput = isPresentingSelectedAssetInput
        self.isPresentingWallets = isPresentingWallets
    }

    public var assets: [AssetData] {
        assetsQuery.value
    }

    var showsDevelopmentAssets: Bool { developmentAssetIds.isNotEmpty }

    var developmentAssets: [AssetData] {
        developmentAssetIds.compactMap { assetId in
            developmentAssetsQuery.value.first { $0.asset.id == assetId }
        }
    }

    func assetData(for asset: WowPayAsset) -> AssetData? {
        fixedAssetsQuery.value.first { $0.asset.id == asset.assetId }
    }

    func balance(for asset: WowPayAsset) -> WalletBalanceViewModel {
        let total = homeState.balance
        let currency = observablePreferences.currency
        let selectedValue = holdingsValue(for: asset, currency: currency)
        return WalletBalanceViewModel(
            primaryValue: total.primaryValue,
            secondaryValue: selectedValue?.doubleValue,
            currency: currency,
            secondaryTitle: asset.rawValue,
            secondaryDecimalValue: selectedValue,
        )
    }

    var homeBalance: WalletBalanceViewModel {
        let value = usdHoldingsValue(for: .bsv)
        return WalletBalanceViewModel(
            primaryValue: value?.doubleValue,
            secondaryValue: value?.doubleValue,
            currency: .usd,
            secondaryTitle: WowPayAsset.bsv.rawValue,
            secondaryDecimalValue: value,
        )
    }

    private func holdingsValue(for asset: WowPayAsset, currency: Currency) -> Decimal? {
        guard let value = usdHoldingsValue(for: asset),
              let rate = service.convertTotalValue(value: 1, currency: currency.rawValue, rates: fiatRates),
              let decimalRate = Decimal(string: String(rate), locale: Locale(identifier: "en_US_POSIX"))
        else { return nil }
        let convertedValue = value * decimalRate
        return convertedValue.isNaN ? nil : convertedValue
    }

    private func usdHoldingsValue(for asset: WowPayAsset) -> Decimal? {
        let priceUsd: Double?
        switch asset {
        case .bsv: priceUsd = bsvPriceUsdQuery.value
        case .trx: priceUsd = trxPriceUsdQuery.value
        case .usdt: priceUsd = usdtPriceUsdQuery.value
        case .mnee:
            // TODO(WOWPAY-MNEE): Replace the placeholder USD value when MNEE balances and pricing are connected.
            return 0
        }
        guard let assetData = assetData(for: asset), !assetData.balance.total.isZero else { return 0 }
        guard let priceUsd, priceUsd > 0,
              let price = Decimal(string: String(priceUsd), locale: Locale(identifier: "en_US_POSIX")),
              let amount = try? ValueFormatter.full.decimal(from: assetData.balance.total, decimals: assetData.asset.decimals.asInt)
        else { return nil }
        let value = amount * price
        return value.isNaN ? nil : value
    }

    var manageTokenTitle: String {
        Localized.Wallet.manageTokenList
    }

    var perpetualsTitle: String {
        Localized.Perpetuals.title
    }

    var collectionsTitle: String {
        Localized.Nft.collections
    }

    var collectionsContent: CollectionsContent {
        collectionsModel.content
    }

    public var searchImage: Image {
        Images.System.search
    }

    public var scannerImage: Image {
        Images.System.qrCodeViewfinder
    }

    public var manageImage: Image {
        Images.Actions.manage
    }

    public var walletBarModel: WalletBarViewViewModel {
        let walletModel = WalletViewModel(wallet: wallet)
        return WalletBarViewViewModel(
            name: walletModel.name,
            image: walletModel.avatarImage,
        )
    }

    var homeState: WalletHomeState {
        let currency = observablePreferences.currency
        let currencyCode = currency.rawValue
        let viewState = service.viewState(
            wallet: wallet,
            balances: fiatValuesQuery.value,
            perpetual: perpetualBalanceQuery.value,
            banners: bannersQuery.value,
            isWalletEmpty: assets.allSatisfy(\.balance.total.isZero),
        )
        let totalValue = service.convertTotalValue(value: viewState.totalValue.value, currency: currencyCode, rates: fiatRates)
        return WalletHomeState(
            header: WalletHeaderViewModel(
                totalValue: viewState.totalValue.map(),
                currencyCode: Currency.usd.rawValue,
                showsPnl: viewState.showsPnl,
                actions: viewState.headerActions,
                walletType: wallet.type,
            ),
            balance: WalletBalanceViewModel(
                primaryValue: totalValue,
                secondaryValue: totalValue,
                currency: currency,
            ),
            currencyCode: currencyCode,
            showPerpetuals: observablePreferences.showPerpetuals(for: wallet),
            showCollections: viewState.showCollections,
            visibleBanners: viewState.visibleBanners.map { $0.map() },
        )
    }

    func bannerContent(for banner: Banner) -> BsvHomeBannerContent {
        service.content(for: banner)
    }

    private var fiatRates: [FiatRate] {
        fiatRatesQuery.value.map { FiatRate(symbol: $0.symbol.rawValue, rate: $0.rate) }
    }
}

// MARK: - Business Logic

public extension WalletSceneViewModel {
    internal func load() async {
        await updateWallet()
    }

    internal func loadOnce() async {
        await loadOnce(wallet: wallet)
    }

    func onSelectWalletBar() {
        isPresentingWallets.wrappedValue = true
    }

    func onSelectManage(chains: [Chain] = []) {
        isPresentingSheet = .selectAsset(.manage, chains: chains)
    }

    func onToggleSearch() {
        isPresentingSearch.toggle()
    }

    func onSelectScanner() {
        if let onRequestScan {
            onRequestScan()
        } else {
            isPresentingScanner = true
        }
    }

    func onSelectAddCustomToken() {
        isPresentingSheet = .addAsset
    }

    internal func onSelectPortfolio() {
        isPresentingSheet = .portfolio(.wallet)
    }

    func onSelectHomeReceive(asset: WowPayAsset) -> AssetData? {
        guard homeState.header.isEnabled(.receive),
              let assetData = currentWalletAssetData(for: asset)
        else {
            isPresentingToastMessage = ToastMessage(title: Localized.wowpayReceiveAssetUnavailable(asset.rawValue), image: "info.circle")
            return nil
        }
        return assetData
    }

    var supportsColdRequests: Bool { bsvColdWalletCanSend(wallet: wallet.map()) }

    func onSelectHomeSend(asset: WowPayAsset) -> AssetData? {
        guard homeState.header.isEnabled(.send),
              wallet.canSign || supportsColdRequests,
              let assetData = currentWalletAssetData(for: asset)
        else {
            isPresentingToastMessage = ToastMessage(title: Localized.wowpaySendAssetUnavailable(asset.rawValue), image: "info.circle")
            return nil
        }
        return assetData
    }

    internal func onHeaderAction(type: HeaderButtonType) {
        switch type {
        case .buy: isPresentingSheet = .selectAsset(.buy, chains: [])
        case .send: isPresentingSheet = .selectAsset(.send(.none), chains: [])
        case .receive: isPresentingSheet = .selectAsset(.receive(.asset), chains: [])
        case .swap: isPresentingSheet = .swap
        case .sell, .more, .stake, .deposit, .withdraw: break
        }
    }

    internal func onSelectWatchWalletInfo() {
        isPresentingSheet = .infoSheet(.watchWallet)
    }

    internal func onBanner(action: BannerAction) {
        switch action.type {
        case .event, .closeBanner:
            Task {
                try await handleBanner(action: action)
            }
        case let .button(bannerButton):
            switch bannerButton {
            case .buy: isPresentingSheet = .selectAsset(.buy, chains: [])
            case .receive: isPresentingSheet = .selectAsset(.receive(.asset), chains: [])
            }
        }
        isPresentingUrl = action.url
    }

    internal func onCopyAddress(_ message: String) {
        isPresentingToastMessage = .copy(message)
    }

    internal func onSelectUnavailableAsset() {
        // TODO(WOWPAY-MNEE): Connect real MNEE metadata, balances and asset actions.
        isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
    }

    func onWalletTabReselected(_: Bool, _: Bool) {
        isPresentingSearch = false
    }

    func onTransferComplete() {
        isPresentingSheet = nil
    }
}

// MARK: - Private

extension WalletSceneViewModel {
    private func currentWalletAssetData(for asset: WowPayAsset) -> AssetData? {
        guard let assetData = assetData(for: asset),
              let account = wallet.accounts.first(where: { $0.chain == assetData.asset.chain }),
              !account.address.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              assetData.account.chain == account.chain,
              assetData.account.address == account.address
        else { return nil }
        return assetData
    }

    private func loadOnce(wallet: Wallet) async {
        let shouldShowLoadingAssets = shouldShowInitialLoadingAssets

        if shouldShowLoadingAssets {
            isLoadingAssets = true
        }

        await updateWallet()

        if shouldShowLoadingAssets, self.wallet.id == wallet.id {
            isLoadingAssets = false
        }
    }

    private func updateWallet() async {
        guard !isRefreshing else { return }
        isRefreshing = true
        refreshState = .loading
        defer { isRefreshing = false }
        do {
            try Task.checkCancellation()
            try await service.refresh()
            try Task.checkCancellation()
            refreshState = .data(())
        } catch {
            refreshState = .error(error)
        }
    }

    private var shouldShowInitialLoadingAssets: Bool {
        (try? service.showsInitialLoading()) ?? false
    }

    private func handleBanner(action: BannerAction) async throws {
        try await service.applyAction(action)
    }

    func setAssetPinned(_ assetId: AssetId, pinned: Bool) async throws {
        try await service.setAssetPinned(assetId: assetId, pinned: pinned)
    }

    func setAssetsEnabled(_ assetIds: [AssetId], enabled: Bool) async throws {
        try await service.setAssetsEnabled(assetIds: assetIds, enabled: enabled)
    }
}
