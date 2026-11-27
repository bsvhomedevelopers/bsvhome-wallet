import Components
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

struct UsdtAssetDetailView<MenuContent: View>: View {
    @Bindable private var model: AssetSceneViewModel
    private let onSelectHeader: (HeaderButtonType) -> Void
    private let menu: MenuContent

    init(model: AssetSceneViewModel, onSelectHeader: @escaping (HeaderButtonType) -> Void, @ViewBuilder menu: () -> MenuContent) {
        self.model = model
        self.onSelectHeader = onSelectHeader
        self.menu = menu()
    }

    var body: some View {
        WowPayAssetDetailView(selectedTab: $model.selectedTab, tabs: [.overview, .transactions, .about], tabTitle: { $0 == .about ? Localized.wowpayTokenInformation : $0.title }) {
            assetHeader
        } content: {
            switch model.selectedTab {
            case .overview: overview
            case .transactions: transactions
            case .about, .address: tokenInformation
            case .multisig: EmptyView()
            }
        }
        .id(model.selectedTab)
    }

    private var assetHeader: some View {
        WowPayAssetDetailHeader(image: model.tokenAssetImage, symbol: model.assetModel.symbol, name: "Tether USD", kind: Localized.wowpayStablecoin) {
            NavigationLink(value: Scenes.Price(asset: model.assetModel.asset)) {
                VStack(alignment: .trailing, spacing: Spacing.space6) {
                    Text(price)
                        .font(.wowPay.assetPrice)
                        .foregroundStyle(Colors.WowPay.heading)
                    Text(change)
                        .font(.wowPay.label)
                        .foregroundStyle(changeColor)
                }
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("price")
        }
        .contextMenu { menu }
    }

    private var overview: some View {
        VStack(alignment: .leading, spacing: .zero) {
            VStack(spacing: Spacing.wowPay.servicePadding) {
                if model.detailsState.showsBanners, let banner = model.visibleBanners.first {
                    BannerView(banner: banner, content: model.bannerContent(for: banner), action: model.onSelectBanner)
                }
                if let status = model.statusViewModel {
                    AssetStatusView(model: status, action: model.onSelectTokenStatus)
                }
                ValueHeaderView(model: model.assetHeaderModel, isPrivacyEnabled: .constant(false), titleActionType: .none, style: .wowPayToken, onHeaderAction: onSelectHeader, networkTitle: model.assetModel.networkFullName, onInfoAction: model.onSelectWalletHeaderInfo)
                HeaderButtonsView(buttons: model.tokenButtons, style: .wowPayTokenActions, action: onSelectHeader)
            }
            section(title: Localized.wowpayAssetOverview) {
                HStack(alignment: .top, spacing: Spacing.wowPay.grid) {
                    metric(title: Localized.wowpayCurrentPrice, value: price) {
                        Text(Localized.wowpayDailyChange(change)).foregroundStyle(changeColor)
                    }
                    metric(title: Localized.wowpayHoldingValue, value: model.assetHeaderModel.subtitle ?? "—") {
                        Text(model.assetDataModel.totalBalanceTextWithSymbol).foregroundStyle(Colors.WowPay.positive)
                    }
                }
                .fixedSize(horizontal: false, vertical: true)
            }
            section(title: Localized.wowpayNetworkInformation, actionTitle: Localized.wowpayViewDetails, action: { model.selectedTab = .about }) {
                networkInformation
            }
            section(title: Localized.wowpayRecentTransactions, actionTitle: Localized.wowpayViewAll, action: { model.selectedTab = .transactions }) {
                transactionRows(model.recentTransactions)
            }
        }
    }

    private var transactions: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.servicePadding) {
            Text(model.assetModel.symbol + " " + Localized.Activity.title)
                .font(.wowPay.sectionTitle).foregroundStyle(Colors.WowPay.heading)
                .accessibilityAddTraits(.isHeader)
            transactionRows(model.transactionSections.flatMap(\.values))
        }
    }

    private func transactionRows(_ transactions: [TransactionViewModel]) -> some View {
        WowPayAssetTransactions(transactions: transactions, currency: model.assetDataModel.currencyCode, state: model.transactionsState, onRetry: { Task { await model.load() } })
    }

    private var tokenInformation: some View {
        VStack(alignment: .leading, spacing: .zero) {
            VStack(alignment: .leading, spacing: Spacing.space4) {
                Text(Localized.wowpayAbout + " " + model.assetModel.symbol)
                    .font(.wowPay.cardTitle).foregroundStyle(Colors.WowPay.heading)
                    .accessibilityAddTraits(.isHeader)
                Text(Localized.wowpayUsdtDescription)
                    .font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .wowPayCard()
            section(title: Localized.wowpayTokenInformation) {
                networkInformation
            }
            if model.detailsState.showsResources {
                section(title: model.resourcesTitle) {
                    VStack(spacing: Spacing.space12) {
                        ListItemView(field: model.energyField, style: .wowPay)
                        ListItemView(field: model.bandwidthField, style: .wowPay)
                    }.wowPayCard()
                }
            }
        }
    }

    private var networkInformation: some View {
        VStack(alignment: .leading, spacing: Spacing.space12) {
            VStack(spacing: .zero) {
                infoRow(title: Localized.wowpayAssetNetwork) {
                    HStack(spacing: Spacing.wowPay.tokenNetworkGap) {
                        AssetImageView(assetImage: model.networkAssetImage, size: Sizing.wowPay.chevron)
                        Text(model.assetModel.networkName)
                    }
                    .foregroundStyle(Colors.WowPay.brand)
                    .padding(.vertical, Spacing.space6).padding(.horizontal, Spacing.space10)
                    .background(Colors.WowPay.serviceSurface, in: Capsule())
                }
                infoDivider
                infoRow(title: Localized.wowpayTokenType) { Text(model.assetData.asset.type.rawValue) }
                infoDivider
                infoRow(title: Localized.wowpayPrecision) { Text(Localized.wowpayPrecisionValue(model.assetData.asset.decimals.description)) }
                infoDivider
                infoRow(title: Localized.wowpayAssetIdentifier) {
                    Text(model.assetData.asset.tokenId ?? "—")
                        .lineLimit(1).truncationMode(.middle)
                        .frame(maxWidth: Sizing.wowPay.tokenIdentifierWidth, alignment: .trailing)
                        .textSelection(.enabled)
                }
            }
            .padding(.horizontal, Spacing.space16).padding(.vertical, Spacing.space4)
            .wowPayCard(padding: .zero, cornerRadius: Sizing.wowPay.rowCorner)
            Text(Localized.wowpayNetworkConsistencyNote)
                .font(.wowPay.caption).foregroundStyle(Colors.WowPay.tokenNoteText)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(Spacing.space12)
                .background(Colors.WowPay.tokenNoteSurface, in: RoundedRectangle(cornerRadius: Sizing.wowPay.pairCorner))
        }
    }

    private var infoDivider: some View { Colors.WowPay.border.frame(height: Spacing.space1) }

    private func infoRow(title: String, @ViewBuilder value: () -> some View) -> some View {
        HStack(spacing: Spacing.space12) {
            Text(title).font(.wowPay.rate).foregroundStyle(Colors.WowPay.secondaryText)
            Spacer(minLength: .zero)
            value().font(.wowPay.transactionValue).foregroundStyle(Colors.WowPay.heading)
                .multilineTextAlignment(.trailing)
        }
        .padding(.vertical, Spacing.wowPay.servicePadding)
    }

    private func metric(title: String, value: String, @ViewBuilder footnote: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: .zero) {
            Text(title).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                .padding(.bottom, Spacing.wowPay.tokenMetricTitleGap)
            Text(value).font(.wowPay.assetTitle).foregroundStyle(Colors.WowPay.heading)
                .lineLimit(1).minimumScaleFactor(0.5)
            footnote().font(.wowPay.rate.weight(.bold)).padding(.top, Spacing.space4)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .wowPayCard(padding: Spacing.wowPay.rowPadding, cornerRadius: Sizing.wowPay.serviceCorner)
    }

    private func section(title: String, actionTitle: String? = nil, action: (() -> Void)? = nil, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.servicePadding) {
            if let actionTitle, let action {
                WowPaySectionHeader(title: title, actionTitle: actionTitle, style: .assetDetail, action: action)
            } else {
                WowPaySectionHeader(title: title)
            }
            content()
        }.padding(.top, Spacing.wowPay.section)
    }

    private var price: String { model.assetDataModel.isPriceAvailable ? model.assetDataModel.priceAmountText : "—" }
    private var change: String { model.isPriceChangeAvailable ? model.assetDataModel.priceChangeText : "—" }
    private var changeColor: Color {
        guard model.isPriceChangeAvailable else { return Colors.WowPay.secondaryText }
        return (model.assetData.price?.priceChangePercentage24h ?? .zero) > .zero ? Colors.WowPay.positive : model.assetDataModel.priceChangeTextColor
    }
}
