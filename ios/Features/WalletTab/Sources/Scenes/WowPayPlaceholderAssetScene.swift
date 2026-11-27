import Components
import Localization
import PrimitivesComponents
import Style
import SwiftUI

struct WowPayPlaceholderAssetScene: View {
    @State private var selectedTab: WowPayAssetDetailTab = .overview
    @State private var isPresentingPaymail = false
    @State private var toast: ToastMessage?

    var body: some View {
        WowPayAssetDetailView(selectedTab: $selectedTab) {
            WowPayAssetDetailHeader(image: .image(WowPayAsset.mnee.image), symbol: WowPayAsset.mnee.rawValue, name: "MNEE", kind: WowPayAsset.mnee.subtitle) {
                VStack(alignment: .trailing, spacing: Spacing.space6) {
                    Text("—").font(.wowPay.assetPrice).foregroundStyle(Colors.WowPay.heading)
                    Text("—").font(.wowPay.label).foregroundStyle(Colors.WowPay.secondaryText)
                }
            }
        } content: {
            switch selectedTab {
            case .overview: overview
            case .address: address
            case .transactions:
                VStack(alignment: .leading, spacing: Spacing.wowPay.servicePadding) {
                    sectionTitle(Localized.Activity.title)
                    transactions
                }
            case .about: about
            case .multisig: EmptyView()
            }
        }
        .navigationTitle(WowPayAsset.mnee.rawValue)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Button(Localized.Wallet.receive) {}.disabled(true)
                    Button(Localized.Activity.title) { selectedTab = .transactions }
                } label: {
                    Text("•••").font(.wowPay.moreDots)
                        .tracking(WowPayTypography.moreDotsTracking)
                }
                .accessibilityLabel(Localized.Wallet.more)
                .accessibilityIdentifier("wowPayAssetMore")
            }
        }
        .toast(message: $toast)
        .navigationDestination(isPresented: $isPresentingPaymail) {
            WowPayUnavailableScene(page: .paymail)
        }
    }

    private var overview: some View {
        WowPayAssetDetailOverview(onPaymail: { isPresentingPaymail = true }, onPaymailHelp: unavailable, onViewAll: { selectedTab = .transactions }) {
            ValueHeaderView(model: MneePlaceholderBalanceModel(), isPrivacyEnabled: .constant(false), titleActionType: .none, style: .wowPayOverview, onHeaderAction: nil, onInfoAction: nil)
        } qrCard: {
            WowPayReceiveQRCodeCard(state: .noData, title: Localized.wowpayReceiveQr, infoTitle: Localized.Common.info, receiveTitle: Localized.Wallet.receive, retryTitle: Localized.Common.tryAgain, onHelp: unavailable, onReceive: {}, onRetry: {})
        } transactions: {
            transactions
        } actions: {
            HeaderButtonsView(buttons: [HeaderButton(type: .send, isEnabled: false), HeaderButton(type: .receive, isEnabled: false)], style: .wowPay)
        }
    }

    private var transactions: some View {
        WowPayAssetTransactions(transactions: [], currency: "USD", emptyTitle: Localized.wowpayUnavailable)
    }

    private var address: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                sectionTitle(Localized.wowpayMyAssetAddress(WowPayAsset.mnee.rawValue))
                Text(Localized.wowpayAddressDescription).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                Text("—")
                    .font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .wowPayCard(padding: Spacing.space12, cornerRadius: Sizing.wowPay.inputCorner, surface: Colors.WowPay.addressSurface)
                    .accessibilityIdentifier("wowPayAssetAddress")
                Button {} label: { Label(Localized.Wallet.copyAddress, systemImage: "doc.on.doc") }
                    .buttonStyle(.wowPaySecondary()).disabled(true)
                Text(Localized.wowpayUnavailable).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
            }.wowPayCard()
            VStack(alignment: .leading, spacing: Spacing.wowPay.servicePadding) {
                HStack {
                    sectionTitle(Localized.wowpayAddressRecords)
                    Spacer()
                    Button(Localized.wowpayAddressAdd, action: unavailable).font(.wowPay.label).foregroundStyle(Colors.WowPay.brand)
                }
                Text(Localized.wowpayUnavailable).font(.wowPay.subtitle).foregroundStyle(Colors.WowPay.secondaryText)
                    .frame(maxWidth: .infinity, alignment: .leading).wowPayCard()
            }
        }
    }

    private var about: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            sectionTitle(Localized.wowpayAbout + " MNEE")
            Text("MNEE").font(.wowPay.subtitle).foregroundStyle(Colors.WowPay.secondaryText)
            ListItemView(title: Localized.Transfer.network, titleStyle: .wowPay.rowTitle, subtitle: "—", subtitleStyle: .wowPay.value)
            Divider().overlay(Colors.WowPay.border)
            ListItemView(title: Localized.Common.type, titleStyle: .wowPay.rowTitle, subtitle: "—", subtitleStyle: .wowPay.value)
            ListItemView(title: Localized.Asset.decimals, titleStyle: .wowPay.rowTitle, subtitle: "—", subtitleStyle: .wowPay.value)
            ListItemView(title: Localized.Asset.tokenId, titleStyle: .wowPay.rowTitle, subtitle: "—", subtitleStyle: .wowPay.caption)
            Text(Localized.wowpayUnavailable).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
        }.wowPayCard()
    }

    private func sectionTitle(_ title: String) -> some View {
        Text(title).font(.wowPay.sectionTitle).foregroundStyle(Colors.WowPay.heading).accessibilityAddTraits(.isHeader)
    }

    private func unavailable() {
        toast = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
    }
}

private struct MneePlaceholderBalanceModel: ValueHeaderViewModel {
    let isWatchWallet = false
    let assetImage: AssetImage? = nil
    let title = "—"
    let amountSymbol: String? = WowPayAsset.mnee.rawValue
    let subtitle: String? = "—"
    let subtitleColor = Colors.WowPay.secondaryText
    let buttons: [HeaderButton] = []
}
