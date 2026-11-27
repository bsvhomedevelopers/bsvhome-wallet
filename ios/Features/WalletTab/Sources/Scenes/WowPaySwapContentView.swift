import Components
import Formatters
import Foundation
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

struct WowPaySwapContentView: View {
    enum Presentation {
        case tab
        case detail
        case acquisition
    }

    @FocusState private var isAmountFocused: Bool
    private let model: WowPayPortalSwapViewModel
    private let walletModel: WalletSceneViewModel
    private let presentation: Presentation
    private let entryAssetNotice: String?
    @State private var toast: ToastMessage?

    init(model: WowPayPortalSwapViewModel, walletModel: WalletSceneViewModel, presentation: Presentation = .tab, entryAssetNotice: String? = nil) {
        self.model = model
        self.walletModel = walletModel
        self.presentation = presentation
        self.entryAssetNotice = entryAssetNotice
    }

    var body: some View {
        @Bindable var model = model
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                if presentation == .tab {
                    WowPayBrandHeader(title: Localized.wowpayBrand, subtitle: Localized.wowpayTagline, mark: Image("wowpay-mark")) {
                        notificationsButton
                    }
                }
                HStack {
                    VStack(alignment: .leading, spacing: Spacing.tiny) {
                        Text(Localized.Wallet.swap).font(.wowPay.title)
                        Text(Localized.wowpaySwapSubtitle)
                            .font(.wowPay.subtitle)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                    }
                    Spacer()
                    Image("wowpay-swap-hero").resizable().scaledToFit()
                        .frame(width: Sizing.wowPay.swapHeroWidth, height: Sizing.wowPay.swapHeroHeight)
                        .accessibilityHidden(true)
                }
                if let entryAssetNotice {
                    Label(entryAssetNotice, systemImage: "info.circle")
                        .font(.wowPay.rate)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .wowPayCard(padding: Spacing.wowPay.servicePadding, cornerRadius: Sizing.wowPay.serviceCorner, surface: Colors.WowPay.pale)
                        .accessibilityIdentifier("wowpay-swap-entry-unsupported")
                }
                connectionStatus
                VStack(spacing: .zero) {
                    assetCard(title: Localized.wowpayFrom, side: .from)
                    if !model.isTargetLocked {
                        Button {
                            isAmountFocused = false
                            model.reverse()
                        } label: {
                            Image("wowpay-switch-assets").resizable().scaledToFit()
                                .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                                .frame(width: Sizing.wowPay.serviceIcon, height: Sizing.wowPay.serviceIcon)
                                .background(Colors.WowPay.surface, in: Circle())
                                .shadow(color: Colors.WowPay.shadow, radius: Sizing.wowPay.shadowRadius, y: Sizing.wowPay.shadowOffset)
                        }
                        .accessibilityLabel(Localized.Wallet.swap)
                        .accessibilityIdentifier("wowpay-swap-reverse")
                        .buttonStyle(.plain)
                        .disabled(!model.canReverse)
                        .padding(.vertical, -Spacing.tiny)
                        .zIndex(1)
                    }
                    assetCard(title: Localized.wowpaySwapEstimatedReceive, side: .to)
                }
                quoteSummary
                if let error = model.quoteError {
                    errorCard(error)
                    Button(Localized.Common.refresh) {
                        Task { await model.retryQuote() }
                    }
                    .buttonStyle(.wowPaySecondary())
                    .disabled(model.isBusy)
                }
                if model.connection == .connected, model.pair == nil || (model.isTargetLocked && model.hasPendingOrder) {
                    Text(model.isTargetLocked && model.hasPendingOrder ? Localized.wowpaySwapAcquisitionPendingOrder : Localized.wowpayUnavailableDescription)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                StateButton(
                    text: Localized.Wallet.swap,
                    type: .wowPayPrimary(model.operation == .creating ? .loading() : model.canCreateOrder ? .normal : .disabled),
                ) {
                    isAmountFocused = false
                    Task { await model.createOrder() }
                }
                .accessibilityIdentifier("wowpay-swap-create-order")
                if model.hasPendingOrder, !model.isTargetLocked {
                    Button(Localized.wowpaySwapRetryOrder) {
                        Task { await model.recoverPendingOrder() }
                    }
                    .buttonStyle(.wowPaySecondary())
                    .disabled(model.isBusy)
                }
                if model.hasPair(from: .bsv, to: .usdt) || model.hasPair(from: .usdt, to: .trx) {
                    quickPairs
                }
                ordersSection
            }
            .foregroundStyle(Colors.WowPay.heading)
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .scrollDismissesKeyboard(.interactively)
        .toolbar {
            if presentation == .detail {
                ToolbarItem(placement: .topBarTrailing) {
                    notificationsButton
                }
            }
            ToolbarItemGroup(placement: .keyboard) {
                Spacer()
                Button(Localized.Common.done) { isAmountFocused = false }
            }
        }
        .wowPaySelectionSheet(isPresented: Binding(
            get: { model.selection.side != nil },
            set: {
                if !$0 {
                    model.selection.side = nil
                }
            },
        )) {
            WowPaySelectionSheet(
                title: Localized.wowpaySelectCoin,
                closeTitle: Localized.Common.cancel,
                closeImage: Image("wowpay-more"),
                items: model.selectableAssets,
                selectedID: model.selection.selectedAsset?.id,
                onSelect: model.selectAsset,
            ) { asset in
                WowPaySelectionRow(title: asset.rawValue, subtitle: asset.subtitle, image: asset.image)
            }
        }
        .navigationDestination(isPresented: Binding(
            get: { model.selectedOrder != nil },
            set: {
                if !$0 {
                    model.selectedOrder = nil
                }
            },
        )) {
            WowPayPortalOrderScene(model: model)
        }
        .onChange(of: model.amount) { _, _ in model.amountChanged() }
        .onChange(of: model.showsAllOrders) { _, _ in
            Task { await model.reloadOrders() }
        }
        .refreshable {
            await model.refresh()
        }
        .toast(message: $toast)
    }

    private var notificationsButton: some View {
        Button(action: unavailable) {
            Image("wowpay-bell").resizable().scaledToFit()
                .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
        }
        .accessibilityLabel(Localized.Settings.Notifications.title)
    }

    private var connectionStatus: some View {
        VStack(alignment: .leading, spacing: Spacing.small) {
            switch model.connection {
            case .idle, .connecting:
                HStack(spacing: Spacing.small) {
                    LoadingView(size: .small, tint: Colors.WowPay.brand)
                    Text(Localized.wowpayWalletConnecting)
                }
            case .connected:
                Label(Localized.wowpayWalletConnected, systemImage: "checkmark.circle")
            case .cancelled:
                Text(Localized.Errors.cancelled)
                Button(Localized.Common.tryAgain, action: model.retryConnection).buttonStyle(.wowPaySecondary())
            case let .failed(message):
                errorCard(message)
                Button(Localized.Common.tryAgain, action: model.retryConnection).buttonStyle(.wowPaySecondary())
            }
        }
        .font(.wowPay.rate)
        .foregroundStyle(Colors.WowPay.secondaryText)
        .frame(maxWidth: .infinity, alignment: .leading)
        .wowPayCard(padding: Spacing.wowPay.servicePadding, cornerRadius: Sizing.wowPay.serviceCorner)
        .accessibilityIdentifier("wowpay-portal-auth-status")
    }

    private func assetCard(title: String, side: WowPaySwapSelectionViewModel.Side) -> some View {
        @Bindable var model = model
        let isInput = side == .from
        let asset = isInput ? model.selection.from : model.selection.to
        let balance = balanceModel(for: asset)
        return VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
            Text(title).font(.wowPay.label)
            HStack(spacing: Spacing.small) {
                assetChoice(asset: asset, side: side)
                    .accessibilityLabel(title)
                    .accessibilityValue(asset.rawValue)
                Spacer(minLength: Spacing.small)
                if isInput {
                    CurrencyInputView(text: $model.amount, currencySymbol: "", currencyPosition: .trailing, secondaryText: "", keyboardType: .decimalPad, style: .wowPaySwap)
                        .focused($isAmountFocused)
                        .disabled(model.isBusy)
                        .accessibilityIdentifier("wowpay-swap-from-amount")
                } else if model.isLoadingQuote {
                    LoadingView(size: .small, tint: Colors.WowPay.brand)
                } else {
                    Text(model.quote?.estimatedToAmountDisplay ?? "—")
                        .font(.wowPay.amount)
                        .minimumScaleFactor(0.6)
                        .lineLimit(1)
                        .accessibilityIdentifier("wowpay-swap-estimated-amount")
                }
            }
            HStack {
                Text(network(for: asset))
                Spacer()
                Text(Localized.Transfer.balance(balance?.availableBalanceTextWithSymbol ?? "—"))
                if isInput {
                    Button(Localized.Common.all) {
                        if let balance {
                            model.useMaximum(balance.availableBalanceInputText)
                        }
                    }
                    .foregroundStyle(Colors.WowPay.brand)
                    .frame(minHeight: Sizing.wowPay.minimumTapTarget)
                    .disabled(!model.canUseMaximum || balance == nil)
                    .accessibilityHint(asset == .bsv ? Localized.wowpaySwapBsvMaxUnavailable : "")
                }
            }
            .font(.wowPay.rate)
            .foregroundStyle(Colors.WowPay.secondaryText)
        }
        .wowPayCard(padding: Spacing.wowPay.swapCardPadding, cornerRadius: Sizing.wowPay.compactCardCorner)
    }

    @ViewBuilder
    private func assetChoice(asset: WowPayAsset, side: WowPaySwapSelectionViewModel.Side) -> some View {
        if model.isTargetLocked, side == .to {
            assetChoiceLabel(asset: asset, side: side)
        } else {
            Button {
                isAmountFocused = false
                model.selection.side = side
            } label: {
                assetChoiceLabel(asset: asset, side: side)
            }
            .buttonStyle(.plain)
            .disabled(model.isBusy || model.assets(for: side).isEmpty)
        }
    }

    private func assetChoiceLabel(asset: WowPayAsset, side: WowPaySwapSelectionViewModel.Side) -> some View {
        HStack(spacing: Spacing.space6) {
            coin(asset, size: Sizing.wowPay.selectionCoin)
            Text(asset.rawValue).font(.wowPay.rowTitle)
            if !model.isTargetLocked || side == .from {
                Image(systemName: "chevron.down").font(.wowPay.caption)
            }
        }
        .padding(.vertical, Spacing.space6)
        .padding(.leading, Spacing.space6)
        .padding(.trailing, Spacing.wowPay.grid)
        .background(Colors.WowPay.selectionSurface, in: Capsule())
    }

    private var quoteSummary: some View {
        VStack(alignment: .leading, spacing: Spacing.small) {
            summaryRow(Localized.wowpayReferenceRate, value: model.quote?.rateDisplay ?? "—")
            summaryRow(Localized.wowpayNetworkFee, value: "—")
            summaryRow(Localized.Common.provider, value: model.quote?.providerDisplayName ?? model.pair?.providerDisplayName ?? "—")
            if let quote = model.quote {
                Text(Localized.Common.minimumValue(quote.minFromAmountDisplay))
                if let maximum = quote.maxFromAmountDisplay {
                    Text(Localized.Common.maximumValue(maximum))
                }
                summaryRow(Localized.Common.expiration, value: RelativeDateFormatter().string(from: Date(timeIntervalSince1970: TimeInterval(quote.expiresAt))))
            }
            Text(Localized.wowpaySwapFloatingRateDescription)
                .font(.wowPay.caption)
                .foregroundStyle(Colors.WowPay.secondaryText)
            Text(Localized.wowpaySwapFeeAtConfirmation)
                .font(.wowPay.caption)
                .foregroundStyle(Colors.WowPay.secondaryText)
        }
        .font(.wowPay.rate)
        .wowPayCard(padding: Spacing.wowPay.servicePadding, cornerRadius: Sizing.wowPay.serviceCorner, surface: Colors.WowPay.pale)
    }

    private var quickPairs: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.servicePadding) {
            Text(Localized.wowpayQuickSwap).font(.wowPay.sectionTitle)
            HStack(alignment: .top, spacing: Spacing.small) {
                if model.hasPair(from: .bsv, to: .usdt) {
                    quickPair(from: .bsv, to: .usdt)
                }
                if model.hasPair(from: .usdt, to: .trx) {
                    quickPair(from: .usdt, to: .trx)
                }
            }
        }
        .padding(.top, Spacing.wowPay.grid)
    }

    private var ordersSection: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
            WowPaySectionHeader(title: Localized.wowpaySwapOrders, actionTitle: model.showsAllOrders ? Localized.Transaction.Status.pending : Localized.Common.all) {
                model.showsAllOrders.toggle()
            }
            if let error = model.orderError {
                errorCard(error)
                Button(Localized.Common.refresh) {
                    Task { await model.reloadOrders() }
                }
                .buttonStyle(.wowPaySecondary())
                .disabled(model.connection != .connected || model.isLoadingOrders || model.isBusy)
            }
            if model.isLoadingOrders {
                LoadingView(size: .small, tint: Colors.WowPay.brand)
            }
            if model.hasNoOrders {
                Text(Localized.Common.noResultsFound)
                    .font(.wowPay.subtitle)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            ForEach(model.orders, id: \.orderId) { order in
                Button {
                    isAmountFocused = false
                    model.selectedOrder = order
                } label: {
                    HStack(alignment: .top, spacing: Spacing.small) {
                        VStack(alignment: .leading, spacing: Spacing.small) {
                            Text(order.fromAmountDisplay ?? order.orderId)
                                .font(.wowPay.rowTitle)
                            Text(WowPayPortalOrderLabels.phase(order.orderStatus))
                                .font(.wowPay.subtitle)
                                .foregroundStyle(Colors.WowPay.secondaryText)
                        }
                        Spacer()
                        VStack(alignment: .trailing, spacing: Spacing.small) {
                            Text(order.estimatedToAmountDisplay ?? "—")
                            Text(RelativeDateFormatter().string(from: Date(timeIntervalSince1970: TimeInterval(order.updatedAt))))
                                .font(.wowPay.caption)
                                .foregroundStyle(Colors.WowPay.secondaryText)
                        }
                    }
                    .multilineTextAlignment(.leading)
                    .wowPayCard(padding: Spacing.wowPay.servicePadding, cornerRadius: Sizing.wowPay.serviceCorner)
                }
                .buttonStyle(.plain)
            }
            if model.hasMoreOrders {
                Button(Localized.Wallet.more) { Task { await model.loadMoreOrders() } }
                    .buttonStyle(.wowPaySecondary())
                    .disabled(model.isLoadingOrders)
            }
        }
        .accessibilityIdentifier("wowpay-swap-orders")
    }

    private func quickPair(from: WowPayAsset, to: WowPayAsset) -> some View {
        Button {
            isAmountFocused = false
            model.selectPair(from: from, to: to)
        } label: {
            VStack(spacing: Spacing.small) {
                HStack(spacing: Spacing.wowPay.pairIconGap) {
                    coin(from, size: Sizing.wowPay.pairCoin)
                    Image("wowpay-arrow-right").resizable().scaledToFit()
                        .frame(width: Sizing.wowPay.pairArrow, height: Sizing.wowPay.pairArrow)
                    coin(to, size: Sizing.wowPay.pairCoin)
                }
                Text("\(from.rawValue) \(Localized.Wallet.swap) \(to.rawValue)")
                    .font(.wowPay.serviceCaption).bold()
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(.horizontal, Spacing.wowPay.pairHorizontalPadding)
            .padding(.vertical, Spacing.wowPay.pairVerticalPadding)
            .wowPayCard(padding: .zero, cornerRadius: Sizing.wowPay.pairCorner, border: Colors.WowPay.pairBorder)
        }
        .buttonStyle(.plain)
        .disabled(model.isBusy)
    }

    private func coin(_ asset: WowPayAsset, size: CGFloat) -> some View {
        asset.image.resizable().scaledToFit()
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }

    private func balanceModel(for asset: WowPayAsset) -> AssetDataViewModel? {
        walletModel.assetData(for: asset).map { AssetDataViewModel(assetData: $0, formatter: .full, currencyCode: Currency.usd.rawValue) }
    }

    private func network(for asset: WowPayAsset) -> String {
        if let quote = model.quote {
            return asset == model.selection.from ? quote.from.network : quote.to.network
        }
        guard let pair = model.pair else { return "—" }
        return asset == model.selection.from ? pair.from.network : pair.to.network
    }

    private func summaryRow(_ title: String, value: String) -> some View {
        HStack(alignment: .top) {
            Text(title)
            Spacer()
            Text(value).bold().multilineTextAlignment(.trailing)
        }
    }

    private func errorCard(_ message: String) -> some View {
        ListItemErrorView(errorTitle: Localized.Errors.errorOccurred, error: NSError(domain: "PortalSwap", code: 0, userInfo: [NSLocalizedDescriptionKey: message]))
    }

    private func unavailable() {
        isAmountFocused = false
        toast = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
    }
}
