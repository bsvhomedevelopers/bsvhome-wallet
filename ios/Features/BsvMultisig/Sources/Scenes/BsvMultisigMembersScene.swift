import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigMembersScene: View {
    @Bindable var model: BsvMultisigViewModel
    let walletId: String

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigMembersRules, subtitle: model.wallet(walletId)?.name ?? "") {
            BsvMultisigIdentityCard(isBusy: model.isWorking || model.isLoading)
            if let wallet = model.wallet(walletId) {
                VStack(spacing: .zero) {
                    BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                    BsvMultisigValueRow(title: Localized.multisigThreshold, value: wallet.rule)
                    BsvMultisigValueRow(title: Localized.multisigWeight, value: Localized.multisigEqualWeight)
                    BsvMultisigValueRow(title: Localized.multisigAllowedActions, value: Localized.multisigTransfer)
                    BsvMultisigValueRow(title: Localized.multisigRuleStatus, value: wallet.statusTitle)
                    if let configurationId = wallet.configurationId {
                        BsvMultisigValueRow(title: Localized.multisigConfigurationId, value: configurationId, onCopy: { model.copy(configurationId) })
                    }
                    if let signingWalletName = model.signingWalletName(wallet) {
                        BsvMultisigValueRow(title: Localized.multisigSigningWallet, value: signingWalletName)
                    }
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                VStack(alignment: .leading, spacing: Spacing.small) {
                    Text(Localized.multisigMembers).font(.headline)
                    ForEach(wallet.members, id: \.slot) { BsvMultisigMemberRow(member: $0, onCopy: model.copy) }
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                BsvMultisigNotice(text: Localized.multisigImmutableNotice, tone: .warning)
                BsvMultisigNotice(text: wallet.configurationStatusTitle, tone: wallet.configurationStatus == .changed ? .error : .warning)
                if wallet.configurationStatus == .needsConfirmation || wallet.configurationStatus == .needsLocalReview {
                    NavigationLink(Localized.multisigConfigurationReview, value: BsvMultisigRoute.configuration(walletId)).buttonStyle(.wowPay())
                }
                if wallet.canInvite {
                    NavigationLink(Localized.multisigContinueInvite, value: BsvMultisigRoute.invite(walletId)).buttonStyle(.wowPay())
                }
                NavigationLink(Localized.multisigBackup, value: BsvMultisigRoute.backup(walletId)).buttonStyle(.wowPaySecondary()).disabled(!wallet.canTransact)
            }
        }
        .bsvMultisigRefresh { await model.refreshWallet(walletId) }
    }
}
