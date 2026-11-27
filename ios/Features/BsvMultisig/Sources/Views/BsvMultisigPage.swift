import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigPage<Content: View>: View {
    @Bindable var model: BsvMultisigViewModel
    @State private var isActive = false
    let title: String
    let subtitle: String
    let navigationBarVisibility: Visibility
    @ViewBuilder let content: () -> Content

    init(model: BsvMultisigViewModel, title: String, subtitle: String, navigationBarVisibility: Visibility = .visible, @ViewBuilder content: @escaping () -> Content) {
        self.model = model
        self.title = title
        self.subtitle = subtitle
        self.navigationBarVisibility = navigationBarVisibility
        self.content = content
    }

    var body: some View {
        BsvMultisigPageLayout(title: title, subtitle: subtitle, navigationBarVisibility: navigationBarVisibility) {
            if model.actionOutcome?.submission == .unknown || (model.actionOutcome?.submission == .submitted && model.actionSyncFailed) {
                if model.actionOutcome?.submission == .unknown {
                    BsvMultisigNotice(text: Localized.multisigUncertain, tone: .warning)
                }
                if let proposalId = model.actionOutcome?.proposalId {
                    BsvMultisigValueRow(title: Localized.multisigProposal, value: proposalId)
                }
                if let transactionId = model.actionOutcome?.transactionId {
                    BsvMultisigValueRow(title: Localized.multisigTransactionId, value: transactionId)
                }
            }
            if model.actionSyncFailed {
                BsvMultisigNotice(text: Localized.multisigActionSyncFailed, tone: .warning)
            }
            content()
        }
        .alert(Localized.Errors.errorOccurred, isPresented: Binding(get: { isActive && model.errorMessage != nil }, set: {
            if !$0 {
                model.errorMessage = nil
            }
        })) {
            Button(Localized.Common.done) { model.errorMessage = nil }
        } message: {
            Text(model.errorMessage ?? "")
        }
        .sheet(isPresented: Binding(get: { isActive && model.isSharing }, set: { model.isSharing = $0 }), onDismiss: model.clearShare) {
            if let url = model.shareFileURL {
                ShareSheet(activityItems: [url])
            }
        }
        .onAppear { isActive = true }
        .onDisappear { isActive = false }
        .toast(isPresenting: $model.isShowingCopy, message: ToastMessage(title: Localized.Common.copiedToClipboard, image: "checkmark"))
    }
}
