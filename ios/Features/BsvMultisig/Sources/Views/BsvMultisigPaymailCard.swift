import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigPaymailCard: View {
    @Bindable var model: BsvMultisigViewModel
    let walletId: String

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.space12) {
            Text(Localized.wowpayReceivePaymailTitle).font(.headline)
            if let settings = model.paymailSettings[walletId] {
                if settings.active {
                    Text(settings.handle).textSelection(.enabled)
                        .accessibilityIdentifier("bsvMultisigReceivePaymail")
                    Button(Localized.Common.copy) { model.copy(settings.handle) }.buttonStyle(.wowPaySecondary())
                    Button(Localized.Common.share) { model.share(settings.handle) }.buttonStyle(.wowPaySecondary())
                } else {
                    Text(Localized.paymailInactive).foregroundStyle(Colors.WowPay.secondaryText)
                }
            } else if model.paymailLoading.contains(walletId) {
                LoadingView()
            } else if model.paymailErrors[walletId] == nil {
                Button(Localized.Common.tryAgain) { Task { await model.loadPaymail(walletId) } }.buttonStyle(.wowPaySecondary())
            }
            if let error = model.paymailErrors[walletId] {
                BsvMultisigNotice(text: error, tone: .error)
                Button(Localized.Common.tryAgain) { Task { await model.loadPaymail(walletId) } }.buttonStyle(.wowPaySecondary())
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
    }
}
