import Localization
import Style
import SwiftUI

struct BsvMultisigSettingsScene: View {
    @Bindable var model: BsvMultisigViewModel
    let walletId: String

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigWalletSettings, subtitle: model.wallet(walletId)?.name ?? "") {
            NavigationLink(Localized.multisigMembersRules, value: BsvMultisigRoute.members(walletId)).buttonStyle(.wowPaySecondary())
            NavigationLink(Localized.multisigBackup, value: BsvMultisigRoute.backup(walletId)).buttonStyle(.wowPaySecondary())
                .disabled(model.wallet(walletId)?.canTransact != true)
            NavigationLink(Localized.wowpayPaymailManage, value: BsvMultisigRoute.paymail(walletId)).buttonStyle(.wowPaySecondary())
                .disabled(model.wallet(walletId)?.canTransact != true)
        }
    }
}
