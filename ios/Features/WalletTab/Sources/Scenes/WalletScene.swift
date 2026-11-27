// Copyright (c). Gem Wallet. All rights reserved.

import Components
import struct BsvHomeCore.BsvHomeAssetRow
import BsvHomeCorePrimitives
import Localization
import NFT
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI

public struct WalletScene: View {
    @State private var isPresentingMnee = false
    @State private var isPresentingBalanceAssetSelection = false
    @Binding private var selectedBalanceAsset: WowPayAsset
    private var model: WalletSceneViewModel
    private let presentation: WalletScenePresentation
    private let onSelectAssets: () -> Void
    private let onSelectSwap: () -> Void
    private let onSelectAsset: (Asset) -> Void
    private let onSelectReceive: () -> Void
    private let onSelectSend: () -> Void
    private let onSelectMultisig: () -> Void
    private let onSelectColdWallet: () -> Void
    private let multisigPendingCount: Int

    public init(model: WalletSceneViewModel, selectedBalanceAsset: Binding<WowPayAsset>, presentation: WalletScenePresentation = .assets, onSelectAssets: @escaping () -> Void = {}, onSelectSwap: @escaping () -> Void = {}, onSelectAsset: @escaping (Asset) -> Void = { _ in }, onSelectReceive: @escaping () -> Void = {}, onSelectSend: @escaping () -> Void = {}, onSelectMultisig: @escaping () -> Void = {}, onSelectColdWallet: @escaping () -> Void = {}, multisigPendingCount: Int = 0) {
        self.model = model
        _selectedBalanceAsset = selectedBalanceAsset
        self.presentation = presentation
        self.onSelectAssets = onSelectAssets
        self.onSelectSwap = onSelectSwap
        self.onSelectAsset = onSelectAsset
        self.onSelectReceive = onSelectReceive
        self.onSelectSend = onSelectSend
        self.onSelectMultisig = onSelectMultisig
        self.onSelectColdWallet = onSelectColdWallet
        self.multisigPendingCount = multisigPendingCount
    }

    public var body: some View {
        Group {
            switch presentation {
            case .home:
                WowPayHomeScene(
                    model: model,
                    onSelectAssets: onSelectAssets,
                    onSelectBalanceAsset: { isPresentingBalanceAssetSelection = true },
                    onSelectReceive: onSelectReceive,
                    onSelectSend: onSelectSend,
                )
            case .assets:
                assetsContent
            }
        }
        .wowPaySelectionSheet(isPresented: $isPresentingBalanceAssetSelection) {
            WowPaySelectionSheet(
                title: Localized.wowpaySelectCoin,
                closeTitle: Localized.Common.cancel,
                closeImage: Image("wowpay-more"),
                items: WowPayAsset.visibleCases,
                selectedID: selectedBalanceAsset.id,
                onSelect: { selectedBalanceAsset = $0 },
            ) { asset in
                WowPaySelectionRow(title: asset.rawValue, subtitle: asset.subtitle, image: asset.image)
            }
        }
    }

    @ViewBuilder
    private var assetsContent: some View {
        @Bindable var preferences = model.observablePreferences
        let state = model.homeState
        let balance = model.balance(for: selectedBalanceAsset)

        List {
            VStack(spacing: Spacing.wowPay.card) {
                WowPayBrandHeader(title: Localized.wowpayBrand, subtitle: Localized.wowpayTagline, mark: Image("wowpay-mark")) {
                    Button(action: model.onSelectScanner) {
                        model.scannerImage
                    }
                    .accessibilityLabel(Localized.Wallet.scan)
                    .accessibilityIdentifier("scan")
                }
                WowPayBalanceCard(
                    title: balance.title,
                    value: balance.value,
                    subtitle: balance.secondaryTitle,
                    secondaryAmount: balance.secondaryAmount,
                    isHidden: $preferences.isHideBalanceEnabled,
                    action: model.onSelectPortfolio,
                    coinImage: selectedBalanceAsset.image,
                    onSelectCoin: { isPresentingBalanceAssetSelection = true },
                )
                if WalletAssetsConfiguration.showsMenuBar {
                    HStack(spacing: Spacing.wowPay.grid) {
                        Button(action: model.onSelectWalletBar) {
                            HStack(spacing: .tiny) {
                                Text(model.wallet.name.isEmpty ? Localized.Wallets.title : model.wallet.name)
                                    .lineLimit(1)
                                Images.System.chevronDown
                            }
                            .font(.wowPay.label)
                            .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget)
                            .contentShape(Rectangle())
                        }
                        .accessibilityLabel(Localized.Wallets.title)
                        .accessibilityIdentifier("walletBar")
                        Spacer(minLength: .zero)
                        Button(action: model.onToggleSearch) {
                            model.searchImage
                                .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget)
                        }
                        .accessibilityLabel(Localized.wowpaySearchAssets)
                        HeaderButtonsView(buttons: state.header.buttons, style: .compactMenu, action: onHeaderAction)
                    }
                    .foregroundStyle(Colors.WowPay.brand)
                }
                if state.header.isWatchWallet {
                    Button(Localized.Wallet.Watch.Tooltip.title, action: model.onSelectWatchWalletInfo)
                        .font(.wowPay.caption)
                }
            }
            .buttonStyle(.plain)
            .cleanListRow()

            if state.showPerpetuals {
                Section {
                    PerpetualsPreviewView(
                        wallet: model.wallet,
                        showBalancePrivacy: $preferences.isHideBalanceEnabled,
                    )
                } header: {
                    HeaderNavigationLinkView(title: model.perpetualsTitle, destination: Scenes.Perpetuals())
                }
                .listRowInsets(.assetListRowInsets)
            }

            if let banner = state.visibleBanners.first {
                Section {
                    BannerView(
                        banner: banner,
                        content: model.bannerContent(for: banner),
                        action: model.onBanner,
                    )
                }
                .listRowInsets(.zero)
            }

            VStack(alignment: .leading, spacing: Spacing.wowPay.assetRowHorizontal) {
                WowPaySectionHeader(title: Localized.wowpayMyAssets)
                if model.isLoadingAssets {
                    LoadingTextView(isAnimating: .constant(true))
                }
                VStack(spacing: Spacing.wowPay.grid) {
                    ForEach(WowPayAsset.visibleCases) { asset in
                        WowPayWalletAssetRow(
                            model: WowPayWalletAssetViewModel(
                                asset: asset,
                                assetData: model.assetData(for: asset),
                                currencyCode: state.currencyCode,
                                showBalancePrivacy: $preferences.isHideBalanceEnabled,
                            ),
                            asset: model.assetData(for: asset)?.asset,
                            accessibilityIdentifier: "wowpay_asset_\(asset.rawValue.lowercased())",
                            onSelectAsset: onSelectAsset,
                            onUnavailable: {
                                if asset == .mnee {
                                    isPresentingMnee = true
                                } else {
                                    model.onSelectUnavailableAsset()
                                }
                            },
                        )
                    }
                }
                if WalletAssetsConfiguration.showsAddAsset {
                    manageTokensButton
                }
            }
            .padding(.top, Spacing.wowPay.section)
            .cleanListRow()

            if model.showsDevelopmentAssets {
                VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
                    WowPaySectionHeader(title: Localized.shastaTestnetTitle)
                    Text(Localized.shastaTestnetNotice)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                    ForEach(model.developmentAssets) { assetData in
                        WowPayWalletAssetRow(
                            model: ListAssetItemViewModel(
                                showBalancePrivacy: $preferences.isHideBalanceEnabled,
                                assetDataModel: AssetDataViewModel(assetData: assetData, formatter: .full, currencyCode: state.currencyCode),
                                row: BsvHomeAssetRow(title: .asset, showsSymbol: true, subtitle: .network, trailing: .balance),
                            ),
                            asset: assetData.asset,
                            accessibilityIdentifier: "shasta_asset_\(assetData.asset.symbol.lowercased())",
                            onSelectAsset: onSelectAsset,
                            onUnavailable: model.onSelectUnavailableAsset,
                        )
                    }
                }
                .padding(.top, Spacing.wowPay.section)
                .cleanListRow()
            }

            if WalletAssetsConfiguration.showsCollections, state.showCollections {
                Section {
                    CollectionsPreviewView(content: model.collectionsContent)
                } header: {
                    HeaderNavigationLinkView(title: model.collectionsTitle, destination: Scenes.Collections())
                }
                .listRowInsets(.assetListRowInsets)
            }
        }
        .listStyle(.plain)
        .contentMargins(.horizontal, Spacing.wowPay.page, for: .scrollContent)
        .contentMargins(.top, Spacing.wowPay.page, for: .scrollContent)
        .scrollContentBackground(.hidden)
        .wowPayBackground()
        .navigationDestination(isPresented: $isPresentingMnee) {
            WowPayPlaceholderAssetScene()
        }
        .listSectionSpacing(.compact)
        .id(model.wallet.id)
        .refreshable {
            await model.load()
        }
        .taskOnce {
            Task { await model.loadOnce() }
        }
    }
}

// MARK: - UI

extension WalletScene {
    private func onHeaderAction(_ action: HeaderButtonType) {
        if model.supportsColdRequests, action == .send {
            onSelectSend()
        } else if model.supportsColdRequests, action == .receive {
            onSelectReceive()
        } else {
            model.onHeaderAction(type: action)
        }
    }

    private var manageTokensButton: some View {
        Button(action: { model.onSelectManage() }) {
            Label(Localized.Wallet.AddToken.title, systemImage: "plus")
                .font(.wowPay.label)
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.wowPaySecondary())
        .accessibilityIdentifier("manage")
    }
}
