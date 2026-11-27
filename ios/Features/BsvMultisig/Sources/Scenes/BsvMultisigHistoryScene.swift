import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigHistoryScene: View {
    @Bindable var model: BsvMultisigViewModel
    let walletId: String

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.Activity.title, subtitle: model.wallet(walletId)?.name ?? "") {
            if let activity = model.activities[walletId] {
                ForEach(activity.items, id: \.transactionId) { item in
                    BsvMultisigActivityCard(model: model, walletId: walletId, item: item)
                }
                if activity.items.isEmpty, !activity.isStale, model.activityErrors[walletId] == nil, !model.activityLoading.contains(walletId) {
                    Text(Localized.Activity.State.Empty.title).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                }
            }
            if model.activityLoading.contains(walletId) {
                LoadingView()
            } else if let error = model.activityErrors[walletId] {
                BsvMultisigNotice(text: error, tone: .error)
                Button(Localized.Common.tryAgain) { Task { await model.retryActivity(walletId) } }.buttonStyle(.wowPaySecondary())
            } else if model.activities[walletId]?.nextCursor != nil {
                Button(Localized.wowpayMore) { Task { await model.loadMoreActivity(walletId) } }.buttonStyle(.wowPaySecondary())
            } else if model.activities[walletId] == nil {
                Button(Localized.Common.tryAgain) { Task { await model.refreshActivity(walletId) } }.buttonStyle(.wowPaySecondary())
            }
        }
        .accessibilityIdentifier("bsvMultisigTransactionHistory")
        .bsvMultisigRefresh { await model.refreshWalletActivity(walletId) }
        .task { await model.loadMoreActivity(walletId) }
    }
}
