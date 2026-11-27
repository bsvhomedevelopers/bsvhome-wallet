import Components
import struct BsvHomeCore.BsvHomeBsvMultisigActivityItem
import Localization
import Style
import SwiftUI

struct BsvMultisigTransactionScene: View {
    @Bindable var model: BsvMultisigViewModel
    let walletId: String
    let transactionId: String

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.Common.details, subtitle: model.wallet(walletId)?.name ?? "") {
            if let item = model.activityItem(walletId: walletId, transactionId: transactionId) {
                VStack(alignment: .leading, spacing: Spacing.space8) {
                    Text(item.direction == .sent ? Localized.Transaction.Title.sent : Localized.Transaction.Title.received)
                        .font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                    Text((item.direction == .sent ? "−" : "+") + model.amount(item.valueSatoshis, symbol: true))
                        .font(.wowPay.multisigTransactionAmount)
                    BsvMultisigValueRow(title: Localized.Transaction.status, value: item.confirmed ? Localized.multisigFinished : Localized.Transaction.Status.pending)
                    if let timestamp = item.timestamp {
                        BsvMultisigValueRow(title: Localized.Transaction.date, value: model.date(timestamp))
                    }
                    BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                    BsvMultisigValueRow(title: Localized.Transfer.networkFee, value: item.feeSatoshis.map { model.amount($0, symbol: true) } ?? Localized.Common.notAvailable)
                    if !item.memo.isEmpty {
                        BsvMultisigValueRow(title: Localized.multisigMemo, value: item.memo)
                    }
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                parties(item)
                VStack(alignment: .leading) {
                    BsvMultisigValueRow(title: Localized.multisigTransactionId, value: item.transactionId, onCopy: { model.copy(item.transactionId) })
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                if let explorer = URL(string: "https://whatsonchain.com/tx/" + item.transactionId) {
                    Link(Localized.Transaction.viewOn("WhatsOnChain"), destination: explorer).buttonStyle(.wowPaySecondary())
                }
            } else if model.activityLoading.contains(walletId) {
                LoadingView()
            } else {
                BsvMultisigNotice(text: model.activityErrors[walletId] ?? Localized.multisigErrorMissingRecord, tone: .error)
                Button(Localized.Common.tryAgain) { Task { await model.refreshActivity(walletId) } }.buttonStyle(.wowPaySecondary())
            }
        }
        .accessibilityIdentifier("bsvMultisigTransactionDetails")
        .task {
            if model.activityItem(walletId: walletId, transactionId: transactionId) == nil {
                await model.refreshActivity(walletId)
            }
        }
    }

    @ViewBuilder
    private func parties(_ item: BsvHomeBsvMultisigActivityItem) -> some View {
        if !item.senderAddresses.isEmpty {
            party(title: Localized.Transaction.sender, values: item.senderAddresses)
        } else if item.senderScriptHashes.isEmpty {
            party(title: Localized.Transaction.sender, values: [Localized.Common.notAvailable])
        }
        if !item.senderScriptHashes.isEmpty {
            party(title: Localized.multisigTransactionSenderScript, values: item.senderScriptHashes)
        }
        if let address = item.recipientAddress {
            party(title: Localized.Transaction.recipient, values: [address])
        }
        if let scriptHash = item.recipientScriptHash {
            party(title: Localized.multisigTransactionRecipientScript, values: [scriptHash])
        }
    }

    private func party(title: String, values: [String]) -> some View {
        VStack(alignment: .leading, spacing: Spacing.space8) {
            Text(title).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
            ForEach(values, id: \.self) { value in
                Text(value).textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
    }
}
