import Components
import Localization
import Style
import SwiftUI

struct WowPayHomeScene: View {
    let model: WalletSceneViewModel
    let onSelectAssets: () -> Void
    let onSelectBalanceAsset: () -> Void
    let onSelectReceive: () -> Void
    let onSelectSend: () -> Void

    var body: some View {
        @Bindable var preferences = model.observablePreferences
        let state = model.homeState
        let balance = model.homeBalance
        WowPayHomeContent(
            header: AnyView(header),
            balance: AnyView(WowPayBalanceCard(
                title: balance.title,
                value: balance.value,
                subtitle: balance.secondaryTitle,
                secondaryAmount: balance.secondaryAmount,
                isHidden: $preferences.isHideBalanceEnabled,
                action: onSelectAssets,
                coinImage: WowPayAsset.bsv.image,
                onSelectCoin: onSelectBalanceAsset,
            )),
            isReceiveEnabled: state.header.isEnabled(.receive),
            isSendEnabled: state.header.isEnabled(.send),
            onSelectScan: model.onSelectScanner,
            onSelectReceive: onSelectReceive,
            onSelectSend: onSelectSend,
            status: refreshStatus,
        )
        .refreshable { await model.load() }
        .taskOnce { Task { await model.loadOnce() } }
    }

    private var header: some View {
        WowPayBrandHeader(title: model.wallet.name.isEmpty ? Localized.Wallets.title : model.wallet.name, subtitle: "", mark: Image("wowpay-mark"), appearance: .walletSelection(onSelect: model.onSelectWalletBar)) {
            Button(action: model.onSelectScanner) {
                Image("wowpay-scan").renderingMode(.template).resizable().scaledToFit()
                    .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                    .foregroundStyle(Colors.WowPay.brand)
            }
            .accessibilityLabel(Localized.Wallet.scan)
            .accessibilityIdentifier("scan")
        }
    }

    private var refreshStatus: AnyView? {
        if model.refreshState.isLoading {
            return AnyView(LoadingView().frame(maxWidth: .infinity))
        } else if model.refreshState.isError {
            return AnyView(StateEmptyView(title: Localized.Errors.balancesActivityOutdated) {
                Button(Localized.Common.tryAgain) { Task { await model.load() } }
                    .buttonStyle(.wowPaySecondary())
            }
            .accessibilityIdentifier("wowPayHomeRefreshError"))
        }
        return nil
    }
}
