import Components
import struct BsvHomeCore.BsvHomeBsvMultisigField
import struct BsvHomeCore.BsvHomeBsvMultisigProposal
import Localization
import Style
import SwiftUI

struct BsvMultisigProposalSummary: View {
    let model: BsvMultisigViewModel
    let proposal: BsvHomeBsvMultisigProposal
    var reviewFields: [BsvHomeBsvMultisigField] = []

    private var additionalFields: [BsvHomeBsvMultisigField] {
        reviewFields.filter { !["amount", "fee", "payment", "configurationId"].contains($0.title) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.small) {
            HStack {
                BsvMultisigChip(title: proposal.statusTitle, confirmed: proposal.state == .confirmed)
                Spacer()
            }
            Text(model.amount(reviewFields.first { $0.title == "amount" }?.value ?? proposal.valueSatoshis, symbol: true))
                .font(.wowPay.multisigTransactionAmount)
                .frame(maxWidth: .infinity)
                .padding(.vertical, Spacing.wowPay.page)
            BsvMultisigValueRow(title: Localized.Common.address, value: reviewFields.first { $0.title == "payment" }?.value ?? proposal.destination)
            BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
            BsvMultisigValueRow(title: Localized.multisigMemo, value: proposal.memo.isEmpty ? "—" : proposal.memo)
            BsvMultisigValueRow(title: Localized.Transfer.networkFee, value: model.amount(reviewFields.first { $0.title == "fee" }?.value ?? proposal.feeSatoshis, symbol: true))
            BsvMultisigValueRow(title: Localized.multisigProposalId, value: proposal.id)
            BsvMultisigValueRow(title: Localized.multisigExpiresAt, value: model.date(proposal.expiresAt))
            if let transactionId = proposal.transactionId {
                BsvMultisigValueRow(title: Localized.multisigTransactionId, value: transactionId)
                BsvMultisigValueRow(title: Localized.multisigConfirmations, value: String(proposal.confirmations ?? 0))
            }
            if let wallet = model.wallet(proposal.walletId) {
                BsvMultisigValueRow(title: Localized.multisigRule, value: wallet.rule)
                if let configurationId = wallet.configurationId {
                    BsvMultisigValueRow(title: Localized.multisigConfigurationId, value: configurationId)
                }
                if let walletName = model.signingWalletName(wallet) {
                    BsvMultisigValueRow(title: Localized.multisigSigningWallet, value: walletName)
                }
            }
            ForEach(additionalFields.indices, id: \.self) { index in
                BsvMultisigValueRow(title: additionalFields[index].localizedTitle, value: additionalFields[index].title == "change" ? model.amount(additionalFields[index].value, symbol: true) : additionalFields[index].localizedValue)
            }
        }
        .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
    }
}
