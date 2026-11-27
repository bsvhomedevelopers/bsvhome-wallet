import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigPublicFundingPage<Content: View>: View {
    @Environment(\.scenePhase) private var phase
    @Bindable var model: BsvMultisigPublicFundingViewModel
    @State private var isActive = false
    let title: String
    @ViewBuilder let content: () -> Content

    var body: some View {
        BsvMultisigPageLayout(title: title, subtitle: model.payer.name) {
            if model.outcome?.submission == .unknown {
                BsvMultisigNotice(text: Localized.multisigUncertain, tone: .warning)
            }
            if model.requiresRecovery, let transactionId = model.outcome?.transactionId {
                BsvMultisigValueRow(title: Localized.multisigTransactionId, value: transactionId)
            }
            if model.syncFailed {
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
        .onAppear { isActive = true }
        .onDisappear { isActive = false }
        .onChange(of: phase) { _, value in
            if value == .background {
                model.cancelActions()
            }
        }
    }
}
