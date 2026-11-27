// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Components
import Formatters
import struct BsvHomeCore.BsvHomeAssetBalance
import protocol BsvHomeCore.BsvHomeAssetDetailsServiceProtocol
import struct BsvHomeCore.BsvHomeAssetDetailsState
import enum BsvHomeCore.BsvHomeAssetNetworkDestination
import enum BsvHomeCore.BsvHomeBalanceRow
import struct BsvHomeCore.BsvHomeBannerContent
import struct BsvHomeCore.BsvHomeBannerContext
import typealias BsvHomeCore.BsvHomeBigUint
import struct BsvHomeCore.BsvHomeBsvAddressRecord
import struct BsvHomeCore.BsvHomeBsvAddressSnapshot
import struct BsvHomeCore.BsvHomeRecipient
import struct BsvHomeCore.BsvHomeTransferData
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI
import UIKit

@Observable
@MainActor
public final class AssetSceneViewModel: Sendable {
    private enum AddressUpdate {
        case add
        case select(UInt32)
    }

    private let service: any BsvHomeAssetDetailsServiceProtocol
    private let preferences: ObservablePreferences
    private let onSwap: ((AssetId) -> Void)?
    private var isRefreshing = false
    private var addressScanTask: Task<Void, Never>?
    private var addressPreparationTask: Task<Void, Never>?
    private var addressPreparationRevision: UInt64 = 0
    private var addressScanProgressTask: Task<Void, Never>?
    private var addressScanRevision: UInt64 = 0
    private var addressSelectionTask: Task<Void, Never>?
    private var copiedAddress: String?

    private(set) var bsvAddressSnapshot: BsvHomeBsvAddressSnapshot?
    private(set) var addressesState: StateViewType<Void> = .noData
    private(set) var isScanningAddresses = false
    private(set) var isUpdatingAddress = false

    public var selectedTab: WowPayAssetDetailTab = .overview
    var transactionsState: StateViewType<Void> = .loading
    var isPresentingCopyToast = false

    private var isPresentingSelectedAssetInput: Binding<SelectedAssetInput?>

    public var isPresentingToastMessage: ToastMessage?
    public var isPresentingAssetSheet: AssetSheetType?

    public var input: AssetSceneInput {
        didSet {
            guard input.wallet.id != oldValue.wallet.id else { return }
            cancelBsvScan(walletId: oldValue.wallet.id)
            isScanningAddresses = false
            bsvAddressSnapshot = nil
            addressesState = .noData
            copiedAddress = nil
            isPresentingCopyToast = false
        }
    }

    public let assetQuery: ObservableQuery<ChainAssetRequest>
    public let bannersQuery: ObservableQuery<BannersRequest>
    public let transactionsQuery: ObservableQuery<MappedRequest<TransactionsRequest, [ListSection<TransactionViewModel>]>>

    public init(
        service: any BsvHomeAssetDetailsServiceProtocol,
        preferences: ObservablePreferences,
        input: AssetSceneInput,
        isPresentingSelectedAssetInput: Binding<SelectedAssetInput?>,
        onSwap: ((AssetId) -> Void)? = nil,
    ) {
        self.service = service
        self.preferences = preferences

        self.input = input
        assetQuery = ObservableQuery(
            input.assetRequest,
            initialValue: ChainAssetData(
                assetData: AssetData.with(asset: input.asset),
                feeAssetData: AssetData.with(asset: input.asset.chain.asset),
            ),
        )
        bannersQuery = ObservableQuery(input.bannersRequest, initialValue: [])
        transactionsQuery = ObservableQuery(input.transactionsRequest, initialValue: [])
        self.isPresentingSelectedAssetInput = isPresentingSelectedAssetInput
        self.onSwap = onSwap
    }

    public var chainAssetData: ChainAssetData {
        assetQuery.value
    }

    public var banners: [Banner] {
        bannersQuery.value
    }

    public var transactionSections: [ListSection<TransactionViewModel>] {
        transactionsQuery.value
    }

    var detailLayout: AssetDetailLayout { AssetDetailLayout(assetId: assetData.asset.id) }

    public var showsPaymail: Bool { assetData.asset.id == Chain.bitcoinSv.asset.id && walletModel.wallet.canSign }

    private var isTestnet: Bool { ChainConfig.config(chain: assetData.asset.chain).isTestnet }

    var showsPriceAlerts: Bool { !isTestnet }

    var tokenButtons: [HeaderButton] {
        let types: [HeaderButtonType] = isTestnet ? [.send, .receive] : [.send, .receive, .swap]
        return types.map { type in
            HeaderButton(type: type, isEnabled: isHeaderActionEnabled(type))
        }
    }

    func isHeaderActionEnabled(_ type: HeaderButtonType) -> Bool {
        switch detailsState.headerActions {
        case .watchOnly: false
        case let .buttons(buttons): buttons.contains { $0.kind.headerButtonType == type && $0.isEnabled }
        }
    }

    var tokenAssetImage: AssetImage {
        let image = assetModel.assetImage
        return AssetImage(type: image.type, imageURL: image.imageURL, placeholder: image.placeholder)
    }

    var recentTransactions: [TransactionViewModel] {
        Array(transactionSections.flatMap(\.values).prefix(3))
    }

    var hasBsvAddresses: Bool { assetData.asset.chain == .bitcoinSv }

    var showsBsvAddressDiscovery: Bool { hasBsvAddresses && wallet.hasMnemonic }

    var addressRecords: [BsvHomeBsvAddressRecord] {
        guard let snapshot = bsvAddressSnapshot else { return [] }
        return snapshot.records.filter { $0.branch == .receive && $0.index < snapshot.receiveCount }
    }

    var canAddBsvAddress: Bool { bsvAddressSnapshot?.canAdd == true }

    var showsBsvRecovery: Bool {
        showsBsvAddressDiscovery && bsvAddressSnapshot != nil
    }

    var address: String {
        hasBsvAddresses ? bsvAddressSnapshot?.currentAddress ?? "" : assetDataModel.address
    }

    var copyModel: CopyTypeViewModel {
        let value = copiedAddress ?? address
        return CopyTypeViewModel(type: .address(assetData.asset, address: value), copyValue: value)
    }

    var assetKind: String {
        switch assetData.asset.id.type {
        case .native: Localized.wowpayMainChainAsset
        case .token: assetModel.networkFullName
        }
    }

    var transferButtons: [HeaderButton] {
        switch detailsState.headerActions {
        case .watchOnly: []
        case let .buttons(buttons): buttons.filter { $0.kind == .send || $0.kind == .receive }
            .map { HeaderButton(type: $0.kind.headerButtonType, isEnabled: $0.isEnabled) }
        }
    }

    var additionalButtons: [HeaderButton] {
        guard !isTestnet else { return [] }
        return switch detailsState.headerActions {
        case .watchOnly: []
        case let .buttons(buttons): buttons.filter { $0.kind != .send && $0.kind != .receive }
            .map { HeaderButton(type: $0.kind.headerButtonType, isEnabled: $0.isEnabled) }
        }
    }

    public var assetData: AssetData {
        chainAssetData.assetData
    }

    private var asset: Asset {
        assetData.asset
    }

    private var wallet: Wallet {
        walletModel.wallet
    }

    public var title: String {
        assetModel.name
    }

    var balancesTitle: String {
        Localized.Asset.balances
    }

    var networkField: ListItemField {
        ListItemField(title: Localized.Transfer.network, value: assetModel.networkFullName)
    }

    var resourcesTitle: String {
        Localized.Asset.resources
    }

    var energyField: ListItemField {
        ListItemField(title: ResourceViewModel(resource: .energy).title, value: feeAssetDataModel.energyText)
    }

    var bandwidthField: ListItemField {
        ListItemField(title: ResourceViewModel(resource: .bandwidth).title, value: feeAssetDataModel.bandwidthText)
    }

    var networkDestination: BsvHomeAssetNetworkDestination? {
        service.networkDestination(assetId: asset.id.identifier)
    }

    var balanceRows: [BsvHomeBalanceRow] {
        let rows = stakeBalance.detailRows(chain: asset.chain.rawValue, isStakeEnabled: StakingConfiguration.isEnabled && assetData.metadata.isStakeEnabled)
        #if DEBUG
            return rows
        #else
            return rows.filter {
                if case .earn = $0 {
                    false
                } else {
                    true
                }
            }
        #endif
    }

    var detailsState: BsvHomeAssetDetailsState {
        service.state(
            wallet: wallet.map(),
            chain: asset.chain.rawValue,
            metadata: assetData.metadata.map(),
            balance: stakeBalance,
            bannerEvents: visibleBanners.map { $0.event.map() },
            price: assetData.price?.price,
            priceAlerts: assetData.priceAlerts.map { $0.map() },
        )
    }

    var pinText: String {
        assetData.metadata.isPinned ? Localized.Common.unpin : Localized.Common.pin
    }

    var pinSystemImage: String {
        assetData.metadata.isPinned ? SystemImage.unpin : SystemImage.pin
    }

    var pinImage: Image {
        Image(systemName: pinSystemImage)
    }

    var enableText: String {
        assetData.metadata.isBalanceEnabled ? Localized.Asset.hideFromWallet : Localized.Asset.addToWallet
    }

    var enableImage: Image {
        Image(systemName: enableSystemImage)
    }

    var enableSystemImage: String {
        assetData.metadata.isBalanceEnabled ? SystemImage.minusCircle : SystemImage.plusCircle
    }

    var showEarnButton: Bool {
        #if DEBUG
            detailsState.showsEarn
        #else
            false
        #endif
    }

    var networkAssetImage: AssetImage {
        AssetIdViewModel(assetId: assetModel.asset.chain.assetId).networkAssetImage
    }

    var assetDataModel: AssetDataViewModel {
        AssetDataViewModel(
            assetData: assetData,
            formatter: ValueFormatter.full,
            currencyCode: preferences.currency.rawValue,
        )
    }

    var isPriceChangeAvailable: Bool {
        assetDataModel.isPriceAvailable && detailsState.showsPriceChange
    }

    var visibleBanners: [Banner] {
        bannerContext.visibleBanners(stored: banners.map { $0.map() }).map { $0.map() }
    }

    func bannerContent(for banner: Banner) -> BsvHomeBannerContent {
        service.bannerContent(event: banner.event.map(), asset: banner.asset?.map())
    }

    private var bannerContext: BsvHomeBannerContext {
        BsvHomeBannerContext(
            wallet: wallet.map(),
            asset: asset.map(),
            isStakeable: StakingConfiguration.isEnabled && assetData.metadata.isStakeEnabled,
            hasStakeBalance: stakedValue > .zero,
            hasAvailableBalance: assetData.balance.available > 0,
            isAssetActivated: assetData.metadata.isActive,
            assetRankScore: assetData.metadata.rankScore,
            isWalletEmpty: false,
        )
    }

    var assetHeaderModel: AssetHeaderViewModel {
        AssetHeaderViewModel(assetDataModel: assetDataModel, state: detailsState)
    }

    public var shareAssetUrl: URL {
        service.deeplinkUrl(deeplink: .asset(assetId: assetDataModel.asset.id.identifier)).asURL!
    }

    public var assetModel: AssetViewModel {
        AssetViewModel(asset: assetData.asset)
    }

    public var walletModel: WalletViewModel {
        WalletViewModel(wallet: input.wallet)
    }

    public var priceAlertsSystemImage: String {
        assetData.isPriceAlertsEnabled ? SystemImage.bellFill : SystemImage.bell
    }

    public var menuItems: [ActionMenuItemType] {
        [.button(title: viewAddressOnTitle, systemImage: SystemImage.globe, action: { self.onSelect(url: self.addressExplorerUrl) }),
         viewTokenOnTitle.map { .button(title: $0, systemImage: SystemImage.globe, action: { self.onSelect(url: self.tokenExplorerUrl) }) },
         .button(title: Localized.Common.share, systemImage: SystemImage.share, action: onSelectShareAsset)].compactMap(\.self)
    }

    var statusViewModel: VerificationStatusViewModel? {
        service.verificationStatus(asset: assetData.asset.map(), rank: assetData.metadata.rankScore).map { VerificationStatusViewModel(status: $0.map()) }
    }

    var priceAlertsViewModel: PriceAlertsViewModel {
        PriceAlertsViewModel(priceAlerts: assetData.priceAlerts)
    }

    var swapAssetType: SelectedAssetType? {
        guard let pair = try? service.swapPair(assetId: assetData.asset.id.identifier, hasBalance: assetData.balance.available > .zero) else { return nil }
        guard pair.receiveAssetId != nil else { return .swap(assetData.asset, nil) }
        return .swap(assetData.asset.chain.asset, assetData.asset)
    }

    func balanceText(_ value: BsvHomeBigUint) -> String {
        assetDataModel.balanceTextWithSymbol(BigInt(value))
    }

    func stakeBalanceText(_ value: BsvHomeBigUint) -> String {
        value == BsvHomeBigUint(BigInt.zero.description) ? aprModel(for: .stake).text : balanceText(value)
    }

    func balanceTitle(for type: StakeProviderType) -> String {
        switch type {
        case .stake: Localized.Wallet.stake
        case .earn: Localized.Common.earn
        }
    }

    func aprModel(for type: StakeProviderType) -> AprViewModel {
        AprViewModel(apr: assetDataModel.apr(for: type) ?? .zero)
    }
}

// MARK: - Business Logic

public extension AssetSceneViewModel {
    internal func onCopyAddress() {
        onCopyAddress(address)
    }

    internal func onCopyAddress(_ value: String) {
        guard value.isNotEmpty else { return }
        copiedAddress = value
        if isPresentingCopyToast {
            copyModel.copy()
        }
        isPresentingCopyToast = true
    }

    internal func prepareBsvAddresses() async {
        guard hasBsvAddresses, !addressesState.isLoading else { return }
        let walletId = input.wallet.id
        let revision = addressScanRevision
        addressesState = .loading
        do {
            try Task.checkCancellation()
            let snapshot = try await service.prepareBsvAddresses(walletId: walletId.id)
            guard input.wallet.id == walletId, addressScanRevision == revision else { return }
            try Task.checkCancellation()
            bsvAddressSnapshot = snapshot
            addressesState = .data(())
        } catch {
            guard input.wallet.id == walletId, addressScanRevision == revision else { return }
            addressesState = .error(error)
        }
    }

    internal func onContinueBsvScan() {
        guard showsBsvRecovery, !addressesState.isLoading else { return }
        _ = startBsvScan(continueScan: true)
    }

    @discardableResult
    internal func onAddBsvAddress() -> Task<Void, Never>? {
        guard canAddBsvAddress else { return nil }
        return updateBsvAddress(.add)
    }

    @discardableResult
    internal func onSelectBsvAddress(index: UInt32) -> Task<Void, Never>? {
        updateBsvAddress(.select(index))
    }

    private func updateBsvAddress(_ update: AddressUpdate) -> Task<Void, Never>? {
        guard hasBsvAddresses, !isUpdatingAddress, !isScanningAddresses, !addressesState.isLoading else { return nil }
        let walletId = input.wallet.id
        let revision = addressScanRevision
        isUpdatingAddress = true
        addressesState = .loading
        let task = Task {
            defer {
                if revision == addressScanRevision {
                    isUpdatingAddress = false
                    addressSelectionTask = nil
                }
            }
            do {
                try Task.checkCancellation()
                let snapshot = switch update {
                case .add: try await service.addBsvAddress(walletId: walletId.id)
                case let .select(index): try await service.selectBsvAddress(walletId: walletId.id, index: index)
                }
                try Task.checkCancellation()
                guard input.wallet.id == walletId, addressScanRevision == revision else { return }
                bsvAddressSnapshot = snapshot
                addressesState = .data(())
            } catch {
                guard input.wallet.id == walletId, addressScanRevision == revision else { return }
                addressesState = .error(error)
            }
        }
        addressSelectionTask = task
        return task
    }

    private func startBsvScan(continueScan: Bool) -> Task<Void, Never>? {
        guard hasBsvAddresses, !isScanningAddresses, !isUpdatingAddress else { return nil }
        let walletId = input.wallet.id
        let generation: UInt64
        do {
            generation = try service.bsvScanGeneration(walletId: walletId.id)
        } catch {
            addressesState = .error(error)
            return nil
        }
        addressScanRevision &+= 1
        let revision = addressScanRevision
        isScanningAddresses = true
        addressesState = .loading
        addressScanTask = Task {
            guard !Task.isCancelled, input.wallet.id == walletId, addressScanRevision == revision else { return }
            let progress = Task {
                do {
                    while !Task.isCancelled {
                        try await Task.sleep(for: .milliseconds(500))
                        if let snapshot = try await service.bsvAddresses(walletId: walletId.id) {
                            guard !Task.isCancelled, input.wallet.id == walletId, addressScanRevision == revision else { return }
                            bsvAddressSnapshot = snapshot
                        }
                    }
                } catch {
                    guard !Task.isCancelled, input.wallet.id == walletId, addressScanRevision == revision else { return }
                    addressesState = .error(error)
                }
            }
            addressScanProgressTask = progress
            defer { progress.cancel() }
            defer {
                if input.wallet.id == walletId, addressScanRevision == revision {
                    isScanningAddresses = false
                    addressScanProgressTask = nil
                    addressScanTask = nil
                }
            }
            do {
                if bsvAddressSnapshot == nil {
                    let snapshot = try await service.prepareBsvAddresses(walletId: walletId.id)
                    try Task.checkCancellation()
                    guard input.wallet.id == walletId, addressScanRevision == revision else { return }
                    bsvAddressSnapshot = snapshot
                }
                let snapshot = if continueScan {
                    try await service.continueBsvScan(walletId: walletId.id, generation: generation)
                } else {
                    try await service.syncBsvAddresses(walletId: walletId.id, generation: generation)
                }
                try Task.checkCancellation()
                guard input.wallet.id == walletId, addressScanRevision == revision else { return }
                bsvAddressSnapshot = snapshot
                addressesState = .data(())
            } catch {
                guard input.wallet.id == walletId, addressScanRevision == revision else { return }
                addressesState = .error(error)
            }
        }
        return addressScanTask
    }

    internal func onCancelBsvScan() {
        cancelBsvScan(walletId: input.wallet.id)
    }

    private func cancelBsvScan(walletId: WalletId) {
        guard hasBsvAddresses else { return }
        addressScanRevision &+= 1
        addressPreparationRevision &+= 1
        addressPreparationTask?.cancel()
        addressPreparationTask = nil
        addressSelectionTask?.cancel()
        addressSelectionTask = nil
        isUpdatingAddress = false
        addressScanProgressTask?.cancel()
        addressScanTask?.cancel()
        addressScanProgressTask = nil
        addressScanTask = nil
        isScanningAddresses = false
        if addressesState.isLoading {
            addressesState = .error(CancellationError())
        }
        do {
            try service.cancelBsvScan(walletId: walletId.id)
        } catch {
            addressesState = .error(error)
        }
    }

    internal func onSelectNotifications() {
        isPresentingToastMessage = ToastMessage(title: Localized.wowpayNoNewNotifications, image: "bell")
    }

    internal func onSelectUnavailable() {
        isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
    }

    internal func loadOnce() {
        addressPreparationTask?.cancel()
        addressPreparationRevision &+= 1
        let revision = addressPreparationRevision
        addressPreparationTask = Task {
            guard !Task.isCancelled, addressPreparationRevision == revision else { return }
            defer {
                if addressPreparationRevision == revision {
                    addressPreparationTask = nil
                }
            }
            await prepareBsvAddresses()
            guard !Task.isCancelled else { return }
            await load()
        }
    }

    internal func load() async {
        await withTaskGroup(of: Void.self) { group in
            group.addTask { await self.refresh() }
            if assetData.priceAlerts.isNotEmpty {
                group.addTask { await self.updatePriceAlerts() }
            }
        }
    }

    func onSelectReceive() {
        onSelectHeader(.receive)
    }

    internal func onSelectHeader(_ buttonType: HeaderButtonType) {
        if buttonType == .swap, let onSwap {
            onSwap(assetData.asset.id)
            return
        }
        let selectType: SelectedAssetType? = switch buttonType {
        case .buy: .buy(assetData.asset, amount: nil)
        case .sell: .sell(assetData.asset, amount: nil)
        case .send: .send(.asset(asset: assetData.asset.map()))
        case .swap: swapAssetType
        case .receive: .receive(.asset)
        case .stake: .stake(assetData.asset)
        case .more, .deposit, .withdraw:
            fatalError()
        }
        guard let selectType, selectType.isAvailableForPresentation else { return }
        isPresentingSelectedAssetInput.wrappedValue = SelectedAssetInput(
            type: selectType,
            assetData: assetData,
        )
    }

    internal func onSelectWalletHeaderInfo() {
        isPresentingAssetSheet = .info(.watchWallet)
    }

    internal func onSelectBanner(_ action: BannerAction) {
        switch action.type {
        case let .event(event):
            switch event {
            case .stake:
                onSelectHeader(.stake)
            case .activateAsset:
                isPresentingAssetSheet = .transfer(
                    BsvHomeTransferData(
                        inputType: .account(assetData.asset, .activate),
                        recipient: BsvHomeRecipient(address: ""),
                        value: BigInt.zero,
                    ),
                )
            case .accountActivation,
                 .accountBlockedMultiSignature,
                 .onboarding:
                Task {
                    try await service.applyBannerAction(key: action.banner.bsvHomeKey, action: action.type.bsvHomeAction)
                }
            case .suspiciousAsset: break
            case .tradePerpetuals:
                UIApplication.shared.open(service.deeplinkBsvHomeUrl(deeplink: .perpetuals).asURL!)
                preferences.isPerpetualEnabled = true
            }
        case let .button(bannerButton):
            switch bannerButton {
            case .buy: onSelectHeader(.buy)
            case .receive: onSelectHeader(.receive)
            }
        case .closeBanner:
            Task {
                try await service.applyBannerAction(key: action.banner.bsvHomeKey, action: action.type.bsvHomeAction)
            }
        }
        onSelect(url: action.url)
    }

    internal func onSelectEarn() {
        isPresentingSelectedAssetInput.wrappedValue = SelectedAssetInput(
            type: .earn(assetData.asset),
            assetData: assetData,
        )
    }

    func onSelectShareAsset() {
        isPresentingAssetSheet = .share
    }

    func onTransferComplete() {
        isPresentingAssetSheet = .none
    }

    func onTogglePriceAlert() {
        Task {
            let enabled = !assetData.isPriceAlertsEnabled
            do {
                try await setPriceAlert(enabled: enabled)
                isPresentingToastMessage = .priceAlert(for: assetData.asset.name, enabled: enabled)
            } catch {
                isPresentingToastMessage = .error(error.localizedDescription)
            }
        }
    }

    func onSelectTokenStatus() {
        guard let status = statusViewModel?.status else { return }
        isPresentingAssetSheet = .info(.assetStatus(status))
    }

    func onSelectPendingUnconfirmedInfo() {
        isPresentingAssetSheet = .info(.pendingUnconfirmedBalance)
    }

    func onSelectPin() {
        let pinned = !assetData.metadata.isPinned
        Task {
            do {
                try await service.setAssetPinned(assetId: asset.id.identifier, pinned: pinned)
                isPresentingToastMessage = .pin(asset.name, pinned: pinned)
            } catch {
                debugLog("onSelectPin error: \(error)")
            }
        }
    }

    func onSelectEnable() {
        Task {
            let enabled = !assetData.metadata.isBalanceEnabled
            do {
                try await service.setAssetsEnabled(assetIds: [asset.id.identifier], enabled: enabled)
                isPresentingToastMessage = .showAsset(visible: enabled)
            } catch {
                debugLog("onSelectEnable error: \(error)")
            }
        }
    }
}

// MARK: - Private

extension AssetSceneViewModel {
    private var addressExplorerUrl: URL {
        addressLink.url
    }

    private var viewAddressOnTitle: String {
        Localized.Asset.viewAddressOn(addressLink.name)
    }

    private var viewTokenOnTitle: String? {
        if let link = tokenLink {
            return Localized.Asset.viewTokenOn(link.name)
        }
        return .none
    }

    private var tokenExplorerUrl: URL? {
        tokenLink?.url
    }

    private var tokenLink: BlockExplorerLink? {
        guard let tokenId = assetModel.asset.tokenId else {
            return .none
        }
        return service.tokenUrl(chain: assetModel.asset.chain.rawValue, address: tokenId).map { $0.map() }
    }

    private var addressLink: BlockExplorerLink {
        service.addressUrl(chain: assetModel.asset.chain.rawValue, address: address).map()
    }

    private var stakeBalance: BsvHomeAssetBalance {
        BsvHomeAssetBalance(assetData.balance, assetId: asset.id, isActive: assetData.metadata.isActive)
    }

    private var stakedValue: BigInt {
        BigInt(stakeBalance.stakedValue(chain: asset.chain.rawValue))
    }

    private var feeAssetDataModel: AssetDataViewModel {
        AssetDataViewModel(
            assetData: chainAssetData.feeAssetData,
            formatter: ValueFormatter.auto,
            currencyCode: preferences.currency.rawValue,
        )
    }

    private func onSelect(url: URL?) {
        guard let url else { return }
        isPresentingAssetSheet = .url(url)
    }

    private func setPriceAlert(enabled: Bool) async throws {
        try await service.setPriceAlert(assetId: assetModel.asset.id.identifier, enabled: enabled)
    }

    private func refresh() async {
        guard !isRefreshing else { return }
        isRefreshing = true
        transactionsState = .loading
        defer { isRefreshing = false }

        if hasBsvAddresses, bsvAddressSnapshot != nil {
            let scan = isScanningAddresses ? addressScanTask : startBsvScan(continueScan: false)
            let revision = addressScanRevision
            await scan?.value
            guard addressScanRevision == revision else {
                transactionsState = .data(())
                return
            }
            if case let .error(error) = addressesState, error.isCancelled {
                transactionsState = .data(())
                return
            }
        }

        let failures = await service.refresh(assetId: assetModel.asset.id.identifier)
        for failure in failures {
            debugLog("asset scene: refresh \(failure.step) failed: \(failure.message)")
        }
        if let failure = failures.first(where: { $0.step == .syncTransactions }) {
            transactionsState = .error(AnyError(failure.message))
        } else {
            transactionsState = .data(())
        }
        if failures.isNotEmpty {
            isPresentingToastMessage = .error(Localized.Errors.balancesActivityOutdated)
        }
    }

    private func updatePriceAlerts() async {
        do {
            try await service.syncPriceAlerts(assetId: asset.id.identifier)
        } catch {
            debugLog("asset scene: price alerts update error \(error)")
            guard !error.isCancelled else { return }
            isPresentingToastMessage = .error(Localized.Errors.errorOccurred)
        }
    }
}
