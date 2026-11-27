import Components
import struct BsvHomeCore.BsvHomeBsvMultisigProposal
import Localization
import Style
import SwiftUI

struct BsvMultisigProposalCard: View {
    let model: BsvMultisigViewModel
    let proposal: BsvHomeBsvMultisigProposal
    var walletName: String?
    var onSelect: (() -> Void)?

    var body: some View {
        Group {
            if let onSelect {
                Button(action: onSelect) { content }
            } else {
                NavigationLink(value: BsvMultisigRoute.proposal(proposal.id)) { content }
            }
        }
        .buttonStyle(.plain)
        .foregroundStyle(Colors.WowPay.heading)
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: Spacing.space12) {
            HStack(alignment: .top) {
                Text(model.amount(proposal.valueSatoshis, symbol: true)).font(.headline)
                Spacer(minLength: .zero)
                BsvMultisigChip(title: proposal.statusTitle, confirmed: proposal.state == .confirmed)
            }
            Text((proposal.memo.isEmpty ? Localized.multisigTransfer : proposal.memo) + " · " + (walletName ?? model.wallet(proposal.walletId)?.name ?? ""))
                .font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
            BsvMultisigSignatureProgress(signedMembers: proposal.signedMembers.count, requiredSignatures: proposal.requiredSignatures)
            HStack {
                Text("\(proposal.signedMembers.count)/\(proposal.requiredSignatures) " + Localized.multisigSigningProgress)
                Spacer()
                Text(Localized.wowpayViewDetails + " ›").foregroundStyle(Colors.WowPay.brand)
            }
            .font(.caption2).foregroundStyle(Colors.WowPay.secondaryText)
            Text(model.date(proposal.createdAt)).font(.caption2).foregroundStyle(Colors.WowPay.secondaryText)
        }
        .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
    }
}
