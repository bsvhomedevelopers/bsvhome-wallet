// Copyright (c). Gem Wallet. All rights reserved.

import Components
import struct BsvHomeCore.BsvHomeBsvAddressRecord
import BsvHomeCorePrimitives
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

public struct AssetScene<MultisigContent: View>: View {
    @Bindable private var model: AssetSceneViewModel
    @Environment(\.connectionStatus) private var connectionStatus
    private let paymailHandle: String?
    private let paymailStatus: String
    private let onPaymail: (() -> Void)?
    private let onSend: (AssetData) -> Void
    private let onReceive: ((AssetData) -> Void)?
    private let onColdWallet: (() -> Void)?
    private let showsMultisig: Bool
    private let multisigPendingCount: Int
    private let multisigContent: MultisigContent
    private let walletHeader: AnyView?

    public init(model: AssetSceneViewModel, paymailHandle: String? = nil, paymailStatus: String = Localized.wowpayPaymailSetup, onPaymail: (() -> Void)? = nil, onSend: @escaping (AssetData) -> Void, onColdWallet: (() -> Void)? = nil, onReceive: ((AssetData) -> Void)? = nil, showsMultisig: Bool = false, multisigPendingCount: Int = 0, walletHeader: AnyView? = nil, @ViewBuilder multisigContent: () -> MultisigContent) {
        self.model = model
        self.paymailHandle = paymailHandle
        self.paymailStatus = paymailStatus
        self.onPaymail = onPaymail
        self.onSend = onSend
        self.onReceive = onReceive
        self.onColdWallet = onColdWallet
        self.showsMultisig = showsMultisig
        self.multisigPendingCount = multisigPendingCount
        self.multisigContent = multisigContent()
        self.walletHeader = walletHeader
    }

    public var body: some View {
        Group {
            switch model.detailLayout {
            case .token:
                UsdtAssetDetailView(model: model, onSelectHeader: onSelectHeader) { assetMenuContent }
            case .asset:
                WowPayAssetDetailView(selectedTab: $model.selectedTab, tabs: visibleTabs, tabBadge: { $0 == .multisig ? multisigPendingCount : nil }, tabLayout: showsMultisig ? .equalWidth : .scrollable, navigationBarVisibility: walletHeader == nil ? .visible : .hidden) {
                    if let walletHeader {
                        walletHeader
                    }
                    if model.assetData.asset.id != Chain.bitcoinSv.asset.id {
                        assetHeader
                    }
                } content: {
                    tabContent
                }
            }
        }
        .navigationTitle(model.assetModel.symbol)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                if model.detailLayout == .token {
                    Button(action: model.onSelectNotifications) {
                        Image("wowpay-bell").renderingMode(.template).resizable().scaledToFit()
                            .foregroundStyle(Colors.WowPay.icon)
                            .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                    }
                    .accessibilityLabel(Localized.Settings.Notifications.title)
                    .accessibilityIdentifier("wowPayTokenNotifications")
                } else {
                    Menu {
                        assetMenuContent
                    } label: {
                        Text("•••").font(.wowPay.moreDots)
                            .tracking(WowPayTypography.moreDotsTracking)
                    }
                    .accessibilityLabel(Localized.Wallet.more)
                    .accessibilityIdentifier("wowPayAssetMore")
                }
            }
        }
        .refreshableTimer(every: connectionStatus.refreshInterval(for: .wallet)) { _ in
            await model.load()
        }
        .task { model.loadOnce() }
        .copyToast(model: model.copyModel, isPresenting: $model.isPresentingCopyToast)
        .toast(message: $model.isPresentingToastMessage)
        .onDisappear(perform: model.onCancelBsvScan)
    }

    private var visibleTabs: [WowPayAssetDetailTab] {
        showsMultisig ? [.overview, .address, .multisig, .transactions, .about] : [.overview, .address, .transactions, .about]
    }

    @ViewBuilder
    private var assetMenuContent: some View {
        if model.assetData.asset.id == Chain.bitcoinSv.asset.id, let onColdWallet {
            Button(Localized.coldWalletTitle, action: onColdWallet)
        }
        Button(Localized.Wallet.receive) { onSelectHeader(.receive) }
            .disabled(model.address.isEmpty || !model.isHeaderActionEnabled(.receive))
        Button(Localized.Activity.title) { model.selectedTab = .transactions }
        if model.showsPriceAlerts {
            Menu {
                Toggle(model.priceAlertsViewModel.priceAlertsTitle, isOn: Binding(
                    get: { model.assetData.isPriceAlertsEnabled },
                    set: { _ in model.onTogglePriceAlert() },
                ))
                NavigationLink(value: Scenes.AssetPriceAlert(asset: model.assetData.asset)) {
                    Text(Localized.Common.manage)
                }
            } label: {
                Label(model.priceAlertsViewModel.priceAlertsTitle, systemImage: model.priceAlertsSystemImage)
            }
        }
        HeaderButtonsView(buttons: model.additionalButtons, style: .menuItems, action: model.onSelectHeader)
        ForEach(model.menuItems) { ActionMenuItemView(item: $0) }
        if model.detailsState.showsManage {
            Button(model.pinText, systemImage: model.pinSystemImage, action: model.onSelectPin)
            Button(model.enableText, systemImage: model.enableSystemImage, action: model.onSelectEnable)
        }
    }

    private func onSelectHeader(_ buttonType: HeaderButtonType) {
        if buttonType == .receive, let onReceive {
            guard model.isHeaderActionEnabled(.receive) else { return }
            onReceive(model.assetData)
            return
        }
        if buttonType == .send,
           assetIdSupportsNewSend(model.assetData.asset.id)
        {
            guard model.isHeaderActionEnabled(.send) else { return }
            onSend(model.assetData)
            return
        }
        model.onSelectHeader(buttonType)
    }

    private func assetIdSupportsNewSend(_ assetId: AssetId) -> Bool {
        assetId == Chain.bitcoinSv.asset.id || assetId == Chain.tron.asset.id || assetId == Chain.tron.defaultAsset(type: .trc20).id
    }

    private var assetHeader: some View {
        WowPayAssetDetailHeader(image: model.assetModel.assetImage, symbol: model.assetModel.symbol, name: model.assetModel.name, kind: model.assetKind) {
            NavigationLink(value: Scenes.Price(asset: model.assetModel.asset)) {
                VStack(alignment: .trailing, spacing: Spacing.space6) {
                    Text(model.assetDataModel.isPriceAvailable ? model.assetDataModel.priceAmountText : "—")
                        .font(.wowPay.assetPrice)
                        .foregroundStyle(Colors.WowPay.heading)
                    Group {
                        if model.isPriceChangeAvailable {
                            HStack(spacing: Spacing.space6) {
                                Text(model.assetDataModel.priceViewModel.priceChangeAmountText)
                                Text(model.assetDataModel.priceChangeText)
                            }
                        } else {
                            Text("—")
                        }
                    }
                    .font(.wowPay.label)
                    .foregroundStyle(marketChangeColor)
                }
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("price")
        }
    }

    private var marketChangeColor: Color {
        guard model.isPriceChangeAvailable else { return Colors.WowPay.secondaryText }
        return model.assetData.price?.priceChangePercentage24h ?? .zero > .zero ? Colors.WowPay.positive : model.assetDataModel.priceChangeTextColor
    }

    @ViewBuilder
    private var tabContent: some View {
        switch model.selectedTab {
        case .overview: overview
        case .address: addressContent
        case .multisig: multisigContent
        case .transactions: transactionsContent
        case .about: aboutContent
        }
    }

    private var overview: some View {
        WowPayAssetDetailOverview(onPaymail: model.showsPaymail ? onPaymail : nil, paymailHandle: paymailHandle, paymailStatus: paymailStatus, paymailInteraction: .wholeCard, onPaymailHelp: { onPaymail?() }, onViewAll: { model.selectedTab = .transactions }) {
            VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
                if model.detailsState.showsBanners, let banner = model.visibleBanners.first {
                    BannerView(banner: banner, content: model.bannerContent(for: banner), action: model.onSelectBanner)
                }
                if let status = model.statusViewModel {
                    AssetStatusView(model: status, action: model.onSelectTokenStatus)
                }
                ValueHeaderView(
                    model: model.assetHeaderModel,
                    isPrivacyEnabled: .constant(false),
                    titleActionType: .none,
                    style: .wowPayOverview,
                    onHeaderAction: onSelectHeader,
                    onInfoAction: model.onSelectWalletHeaderInfo,
                )
            }
        } qrCard: {
            EmptyView()
        } transactions: {
            transactionRows(model.recentTransactions)
        } actions: {
            HeaderButtonsView(buttons: model.transferButtons, style: .wowPay, action: onSelectHeader)
        }
    }

    private var addressContent: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                Text(Localized.wowpayMyAssetAddress(model.assetModel.symbol))
                    .font(.wowPay.cardTitle)
                    .foregroundStyle(Colors.WowPay.heading)
                    .accessibilityAddTraits(.isHeader)
                Text(Localized.wowpayAddressDescription)
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                addressText
                Button(Localized.Wallet.copyAddress, action: model.onCopyAddress)
                    .buttonStyle(.wowPaySecondary())
                    .disabled(model.address.isEmpty)
            }
            .wowPayCard()
            Button { onSelectHeader(.receive) } label: {
                Text(Localized.Wallet.receive)
                    .font(.wowPay.balanceFootnote.weight(.medium))
                    .foregroundStyle(Colors.WowPay.brand)
                    .padding(.horizontal, Spacing.space12)
                    .frame(minHeight: Sizing.wowPay.minimumTapTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(!model.isHeaderActionEnabled(.receive))
            VStack(alignment: .leading, spacing: Spacing.wowPay.servicePadding) {
                HStack {
                    Text(Localized.wowpayAddressRecords)
                        .font(.wowPay.cardTitle)
                        .foregroundStyle(Colors.WowPay.heading)
                        .accessibilityAddTraits(.isHeader)
                    Spacer()
                    if model.canAddBsvAddress {
                        Button(Localized.wowpayAddressAdd) { model.onAddBsvAddress() }
                            .font(.wowPay.label)
                            .foregroundStyle(Colors.WowPay.brand)
                            .disabled(model.addressesState.isLoading || model.isUpdatingAddress || model.isScanningAddresses)
                            .accessibilityIdentifier("wowPayBsvAddAddress")
                    }
                }
                if model.hasBsvAddresses {
                    bsvAddressRecords
                } else if model.address.isNotEmpty {
                    Button(action: model.onCopyAddress) {
                        HStack(spacing: Spacing.space12) {
                            Image("wowpay-address-book").resizable().scaledToFit()
                                .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                            Text(model.address.preventingHyphenation)
                                .font(.wowPay.caption)
                                .foregroundStyle(Colors.WowPay.secondaryText)
                                .fixedSize(horizontal: false, vertical: true)
                            Spacer(minLength: .zero)
                            Image(systemName: "doc.on.doc")
                                .foregroundStyle(Colors.WowPay.brand)
                        }
                        .wowPayCard()
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private var bsvAddressRecords: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.servicePadding) {
            if model.addressesState.isLoading {
                LoadingView()
            }
            if case let .error(error) = model.addressesState {
                StateEmptyView(title: error.isCancelled ? Localized.Errors.cancelled : Localized.Errors.errorOccurred) {
                    Button(Localized.Common.tryAgain, action: model.loadOnce)
                        .buttonStyle(.wowPaySecondary())
                }
            }
            if let snapshot = model.bsvAddressSnapshot {
                if model.showsBsvRecovery {
                    Text(snapshot.discovery.statusTitle)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                    Text(Localized.wowpayBsvScanProgress(snapshot.receiveScanned.description, snapshot.changeScanned.description))
                        .font(.wowPay.caption)
                    Text(Localized.wowpayBsvScanRangeNotice)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                    if model.isScanningAddresses {
                        Button(Localized.Common.cancel, action: model.onCancelBsvScan)
                            .buttonStyle(.wowPaySecondary())
                    } else {
                        Button(Localized.wowpayBsvContinueScan, action: model.onContinueBsvScan)
                            .buttonStyle(.wowPaySecondary())
                            .disabled(model.isUpdatingAddress || model.addressesState.isLoading)
                    }
                }
                if !snapshot.canAdd {
                    Text(snapshot.allocationPaused ? Localized.wowpayBsvAddressAllocationPaused : model.showsBsvAddressDiscovery ? Localized.wowpayBsvAddressLimitNotice : Localized.wowpayBsvSingleAddressNotice)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                LazyVStack(spacing: Spacing.wowPay.servicePadding) {
                    ForEach(model.addressRecords, id: \.recordIdentifier) { record in
                        bsvAddressRow(record)
                    }
                }
            }
        }
    }

    private func bsvAddressRow(_ record: BsvHomeBsvAddressRecord) -> some View {
        let title = Localized.wowpayBsvReceiveAddressIndex((UInt64(record.index) + 1).formatted(.number.grouping(.never)))
        return HStack(alignment: .center, spacing: Spacing.wowPay.servicePadding) {
            Button {
                guard !record.isCurrent else { return }
                model.onSelectBsvAddress(index: record.index)
            } label: {
                HStack(alignment: .center, spacing: Spacing.wowPay.servicePadding) {
                    Image(record.isCurrent ? "wowpay-check-circle" : "wowpay-address-book")
                        .renderingMode(.template)
                        .resizable().scaledToFit()
                        .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                        .foregroundStyle(Colors.WowPay.brand)
                        .accessibilityHidden(true)
                    VStack(alignment: .leading, spacing: Spacing.space6) {
                        Text(title)
                            .font(.wowPay.sectionAction)
                            .foregroundStyle(Colors.WowPay.heading)
                        Text("\(record.address.prefix(9))…\(record.address.suffix(6))")
                            .font(.wowPay.caption)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(model.addressesState.isLoading || model.isUpdatingAddress || model.isScanningAddresses)
            .accessibilityIdentifier("wowPayBsvSelectAddress_" + record.index.description)
            .accessibilityLabel(title)
            .accessibilityValue(record.address)
            .accessibilityAddTraits(record.isCurrent ? .isSelected : [])
            Button { model.onCopyAddress(record.address) } label: {
                Image(systemName: "doc.on.doc")
                    .foregroundStyle(Colors.WowPay.brand)
                    .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Localized.Wallet.copyAddress)
            .accessibilityValue(record.address)
        }
        .wowPayCard()
        .accessibilityIdentifier("wowPayBsvAddress_" + record.address)
    }

    private var addressText: some View {
        Text(model.address.isEmpty ? "—" : model.address.preventingHyphenation)
            .font(.wowPay.caption)
            .foregroundStyle(Colors.WowPay.heading)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Spacing.space12)
            .background(Colors.WowPay.addressSurface, in: RoundedRectangle(cornerRadius: Spacing.space12))
            .accessibilityIdentifier("wowPayAssetAddress")
    }

    private var transactionsContent: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.servicePadding) {
            sectionTitle(Localized.Activity.title)
            transactionRows(model.transactionSections.flatMap(\.values))
        }
    }

    private func transactionRows(_ transactions: [TransactionViewModel]) -> some View {
        WowPayAssetTransactions(transactions: transactions, currency: model.assetDataModel.currencyCode, state: model.transactionsState, onRetry: { Task { await model.load() } })
    }

    @ViewBuilder
    private var aboutContent: some View {
        if model.assetData.asset.id == Chain.bitcoinSv.asset.id {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                sectionTitle(Localized.wowpayAboutBsvTitle)
                Text(Localized.wowpayAboutBsvBody)
                    .font(.wowPay.body)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .wowPayCard()
        } else {
            assetMetadata
        }
    }

    private var assetMetadata: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                sectionTitle(Localized.wowpayAbout + " " + model.assetModel.symbol)
                Text(model.assetModel.name)
                    .font(.wowPay.subtitle)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                networkView
                Divider().overlay(Colors.WowPay.border)
                ListItemView(title: Localized.Common.type, titleStyle: .wowPay.rowTitle, subtitle: model.assetData.asset.id.type == .native ? Localized.wowpayMainChainAsset : model.assetData.asset.type.rawValue, subtitleStyle: .wowPay.value)
                ListItemView(title: Localized.Asset.decimals, titleStyle: .wowPay.rowTitle, subtitle: model.assetData.asset.decimals.description, subtitleStyle: .wowPay.value)
                if let tokenId = model.assetData.asset.tokenId {
                    ListItemView(title: Localized.Asset.tokenId, titleStyle: .wowPay.rowTitle, subtitle: tokenId, subtitleStyle: .wowPay.caption)
                }
            }
            .wowPayCard()
            balanceDetails
            if model.detailsState.showsResources {
                VStack(alignment: .leading, spacing: Spacing.space12) {
                    sectionTitle(model.resourcesTitle)
                    ListItemView(field: model.energyField, style: .wowPay)
                    ListItemView(field: model.bandwidthField, style: .wowPay)
                }
                .wowPayCard()
            }
        }
    }

    private var networkView: some View {
        Group {
            switch model.networkDestination {
            case let .asset(asset):
                NavigationLink(value: Scenes.Asset(asset: asset.map())) { networkRow }
            case let .assets(chain):
                NavigationLink(value: Scenes.NetworkAssets(chain: Chain(core: chain))) { networkRow }
            case nil: networkRow
            }
        }
        .buttonStyle(.plain)
    }

    private var networkRow: some View {
        ListItemImageView(title: model.networkField.title.text, subtitle: model.networkField.value.text, assetImage: model.networkAssetImage, imageSize: .list.image, style: .wowPay)
    }

    @ViewBuilder
    private var balanceDetails: some View {
        if model.balanceRows.isNotEmpty || model.showEarnButton {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                sectionTitle(model.balancesTitle)
                ForEach(model.balanceRows, id: \.self) { row in
                    switch row {
                    case let .available(value):
                        ListItemView(title: model.assetDataModel.availableBalanceTitle, titleStyle: .wowPay.rowTitle, subtitle: model.balanceText(value), subtitleStyle: .wowPay.value)
                    case let .staked(value):
                        Button { model.onSelectHeader(.stake) } label: {
                            ListItemView(title: model.balanceTitle(for: .stake), titleStyle: .wowPay.rowTitle, subtitle: model.stakeBalanceText(value), subtitleStyle: .wowPay.value)
                        }
                        .disabled(!StakingConfiguration.isEnabled)
                    case let .earn(value):
                        Button(action: model.onSelectEarn) {
                            ListItemView(title: model.balanceTitle(for: .earn), titleStyle: .wowPay.rowTitle, subtitle: model.balanceText(value), subtitleStyle: .wowPay.value)
                        }
                    case let .pendingUnconfirmed(value):
                        ListItemView(title: model.assetDataModel.pendingUnconfirmedBalanceTitle, titleStyle: .wowPay.rowTitle, subtitle: model.balanceText(value), subtitleStyle: .wowPay.value, infoAction: model.onSelectPendingUnconfirmedInfo)
                    case let .reserved(value, url):
                        if let url = url.flatMap(URL.init) {
                            SafariNavigationLink(url: url) {
                                ListItemView(title: model.assetDataModel.reservedBalanceTitle, titleStyle: .wowPay.rowTitle, subtitle: model.balanceText(value), subtitleStyle: .wowPay.value)
                            }
                        } else {
                            ListItemView(title: model.assetDataModel.reservedBalanceTitle, titleStyle: .wowPay.rowTitle, subtitle: model.balanceText(value), subtitleStyle: .wowPay.value)
                        }
                    }
                }
                if model.showEarnButton {
                    Button(action: model.onSelectEarn) {
                        ListItemView(title: model.balanceTitle(for: .earn), titleStyle: .wowPay.rowTitle, subtitle: model.aprModel(for: .earn).text, subtitleStyle: model.aprModel(for: .earn).subtitle.style)
                    }
                }
            }
            .buttonStyle(.plain)
            .wowPayCard()
        }
    }

    private func sectionTitle(_ title: String) -> some View {
        Text(title).font(.wowPay.sectionTitle)
            .foregroundStyle(Colors.WowPay.heading)
            .accessibilityAddTraits(.isHeader)
    }
}
