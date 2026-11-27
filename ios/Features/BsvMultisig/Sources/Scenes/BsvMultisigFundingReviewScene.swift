import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigFundingReviewScene: View {
    @Environment(NavigationPathState.self) private var navigationOwner: NavigationPathState?
    @Environment(BsvMultisigNavigation.self) private var navigation
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @Bindable var model: BsvMultisigViewModel
    let preparationId: String
    @State private var hasReviewed = false

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigFundingReview, subtitle: "") {
            if let preparation = model.fundingPreparation, preparation.id == preparationId, let walletId = preparation.memberWalletId {
                BsvMultisigFundingReviewContent(preparation: preparation, amounts: model.amounts)
                BsvMultisigNotice(text: Localized.multisigFundingReviewNotice, tone: .warning)
                Toggle(Localized.multisigReviewCheck, isOn: $hasReviewed).font(.caption).tint(Colors.WowPay.brand)
                TimelineView(.periodic(from: .now, by: 1)) { _ in
                    if model.fundingRemainingSeconds == 0 {
                        BsvMultisigNotice(text: Localized.multisigFundingExpired, tone: .warning)
                    }
                    StateButton(text: Localized.multisigFundingConfirm, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : hasReviewed && model.fundingRemainingSeconds > 0 ? .normal : .disabled)) {
                        hasReviewed = false
                        model.submitFunding(preparationId, navigation: navigationOwner) { _ in
                            navigation.replaceLast(.fundingResult(walletId))
                        }
                    }
                }
            } else {
                BsvMultisigNotice(text: Localized.multisigFundingExpired, tone: .warning)
            }
            Button(Localized.Common.cancel) { dismiss() }.buttonStyle(.wowPaySecondary()).disabled(model.isWorking)
        }
        .onChange(of: scenePhase) { _, phase in
            hasReviewed = false
            if phase == .background {
                model.cancelActions()
            }
        }
        .onChange(of: model.actionRevision) { _, _ in hasReviewed = false }
    }
}
