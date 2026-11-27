import Components
import enum BsvHomeCore.BsvHomeBsvMultisigError
import Localization
import Style
import SwiftUI
import UIKit

struct BsvMultisigPublicFundingResultScene: View {
    @Environment(NavigationPathState.self) private var navigation: NavigationPathState?
    @Bindable var model: BsvMultisigPublicFundingViewModel
    @State private var copied = false
    let operationId: String
    let onClose: @MainActor () -> Void

    var body: some View {
        BsvMultisigPublicFundingPage(model: model, title: Localized.multisigFundingResult) {
            if let result = model.result, result.operationId == operationId {
                BsvMultisigFundingResultContent(result: result, targetName: result.targetName(memberName: { _ in nil }), amounts: model.amounts) {
                    UIPasteboard.general.string = result.transactionId
                    copied = true
                }
                BsvMultisigValueRow(title: Localized.multisigFundingOperation, value: result.operationId)
                if let code = result.errorCode {
                    BsvMultisigNotice(text: BsvHomeBsvMultisigError.Api(code: code).localizedMessage, tone: .warning)
                }
                if result.status == .pending {
                    BsvMultisigNotice(text: Localized.multisigFundingPendingNotice, tone: .warning)
                    StateButton(text: Localized.multisigFundingRetry, type: .wowPayPrimary(model.isWorking ? .loading() : .normal)) {
                        model.retry(operationId, navigation: navigation)
                    }
                    .accessibilityIdentifier("bsvPublicFundingRetry")
                } else {
                    BsvMultisigNotice(text: Localized.multisigFundingBroadcastNotice)
                    if model.syncFailed {
                        BsvMultisigNotice(text: Localized.multisigFundingSyncFailed, tone: .warning)
                        Button(Localized.multisigRefresh) { model.retrySync(operationId) }.buttonStyle(.wowPaySecondary()).disabled(model.readTask != nil)
                    }
                }
            } else {
                BsvMultisigNotice(text: Localized.multisigErrorMissingRecord, tone: .warning)
            }
            Button(Localized.Common.done, action: onClose).buttonStyle(.wowPaySecondary())
        }
        .accessibilityIdentifier("bsvPublicFundingResult")
        .toast(isPresenting: $copied, message: ToastMessage(title: Localized.Common.copiedToClipboard, image: "checkmark"))
    }
}
