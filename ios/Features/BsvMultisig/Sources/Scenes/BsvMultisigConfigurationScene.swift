import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigConfigurationScene: View {
    @Environment(\.scenePhase) private var scenePhase
    @Bindable var model: BsvMultisigViewModel
    let walletId: String
    @State private var hasReviewed = false

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigConfigurationReview, subtitle: model.wallet(walletId)?.name ?? "") {
            if let wallet = model.wallet(walletId) {
                VStack(alignment: .leading, spacing: Spacing.small) {
                    BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                    BsvMultisigValueRow(title: Localized.multisigThreshold, value: wallet.rule)
                    if let configurationId = wallet.configurationId {
                        BsvMultisigValueRow(title: Localized.multisigConfigurationId, value: configurationId, onCopy: { model.copy(configurationId) })
                    }
                    ForEach(wallet.members, id: \.slot) { BsvMultisigMemberRow(member: $0, onCopy: model.copy) }
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                BsvMultisigNotice(text: wallet.configurationStatusTitle, tone: wallet.configurationStatus == .changed ? .error : .warning)
                if wallet.configurationStatus == .needsConfirmation || wallet.configurationStatus == .needsLocalReview {
                    Toggle(Localized.multisigConfigurationCheck, isOn: $hasReviewed).font(.caption).tint(Colors.WowPay.brand)
                    StateButton(text: wallet.configurationStatus == .needsLocalReview ? Localized.multisigLocalReview : Localized.Transfer.confirm, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : hasReviewed ? .normal : .disabled)) {
                        let version = wallet.version
                        Task {
                            _ = await model.approveConfiguration(walletId, expectedVersion: version)
                            hasReviewed = false
                        }
                    }
                }
            }
        }
        .bsvMultisigRefresh { await model.refreshWallet(walletId) }
        .onChange(of: model.wallet(walletId)?.version) { _, _ in hasReviewed = false }
        .onChange(of: model.wallet(walletId)?.configurationStatus) { _, _ in hasReviewed = false }
        .onChange(of: scenePhase) { _, _ in hasReviewed = false }
    }
}
