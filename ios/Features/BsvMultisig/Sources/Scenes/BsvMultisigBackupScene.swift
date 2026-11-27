import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigBackupScene: View {
    @Bindable var model: BsvMultisigViewModel
    let walletId: String

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigBackup, subtitle: model.wallet(walletId)?.name ?? "") {
            if let wallet = model.wallet(walletId) {
                VStack(alignment: .leading, spacing: Spacing.space12) {
                    BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                    BsvMultisigValueRow(title: Localized.multisigRule, value: wallet.rule)
                    BsvMultisigValueRow(title: Localized.multisigMemberCount, value: String(wallet.memberCount))
                    Text(Localized.multisigBackupDescription).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                BsvMultisigNotice(text: Localized.multisigBackupNotice, tone: .warning)
                Button(Localized.multisigExportConfiguration) { Task { await model.exportBackup(walletId) } }
                    .buttonStyle(.wowPay())
                    .disabled(!wallet.canTransact)
                if !wallet.canTransact {
                    BsvMultisigNotice(text: wallet.configurationStatusTitle)
                }
            }
        }
        .bsvMultisigRefresh { await model.refreshWallet(walletId) }
    }
}
