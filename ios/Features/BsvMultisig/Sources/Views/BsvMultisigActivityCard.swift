import Components
import struct BsvHomeCore.BsvHomeBsvMultisigActivityItem
import Localization
import Style
import SwiftUI

struct BsvMultisigActivityCard: View {
    let model: BsvMultisigViewModel
    let walletId: String
    let item: BsvHomeBsvMultisigActivityItem

    var body: some View {
        NavigationLink(value: BsvMultisigRoute.transaction(walletId: walletId, transactionId: item.transactionId)) {
            VStack(alignment: .leading, spacing: Spacing.space8) {
                HStack(alignment: .top) {
                    Text((item.direction == .sent ? "−" : "+") + model.amount(item.valueSatoshis, symbol: true))
                        .font(.wowPay.rowTitle)
                    Spacer(minLength: .zero)
                    BsvMultisigChip(title: item.confirmed ? Localized.multisigFinished : Localized.Transaction.Status.pending, confirmed: item.confirmed)
                }
                HStack {
                    Text(item.direction == .sent ? Localized.Transaction.Title.sent : Localized.Transaction.Title.received)
                        .font(.wowPay.caption)
                    Spacer(minLength: .zero)
                    if let timestamp = item.timestamp {
                        Text(model.date(timestamp)).font(.wowPay.serviceCaption)
                    }
                }
                .foregroundStyle(Colors.WowPay.secondaryText)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
        }
        .buttonStyle(.plain)
        .foregroundStyle(Colors.WowPay.heading)
        .accessibilityIdentifier("bsvMultisigTransaction_\(item.transactionId)")
    }
}
