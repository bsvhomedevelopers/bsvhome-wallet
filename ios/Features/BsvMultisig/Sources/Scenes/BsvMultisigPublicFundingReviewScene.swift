import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigPublicFundingReviewScene: View {
    @Environment(NavigationPathState.self) private var navigation: NavigationPathState?
    @Environment(\.dismiss) private var dismiss
    @Bindable var model: BsvMultisigPublicFundingViewModel
    let preparationId: String

    var body: some View {
        BsvMultisigPublicFundingPage(model: model, title: Localized.multisigFundingReview) {
            if let review = model.preparation, review.id == preparationId {
                BsvMultisigFundingReviewContent(preparation: review, amounts: model.amounts)
                BsvMultisigNotice(text: Localized.multisigFundingReviewNotice, tone: .warning)
                Toggle(Localized.multisigReviewCheck, isOn: Binding(get: { model.hasReviewed && model.preparation?.id == preparationId }, set: { model.setReviewed($0, preparationId: preparationId) }))
                    .font(.caption).tint(Colors.WowPay.brand)
                    .accessibilityIdentifier("bsvPublicFundingReviewed")
                TimelineView(.periodic(from: .now, by: 1)) { _ in
                    if model.remainingSeconds == 0 {
                        BsvMultisigNotice(text: Localized.multisigFundingExpired, tone: .warning)
                    }
                    StateButton(text: Localized.multisigFundingConfirm, type: .wowPayPrimary(model.isWorking ? .loading() : model.canSubmit(preparationId) ? .normal : .disabled)) {
                        model.submit(preparationId, navigation: navigation) { saved in
                            guard let navigation else { return }
                            var path = navigation.binding.wrappedValue
                            path.removeLast()
                            path.append(BsvMultisigPublicFundingRoute.result(saved.operationId))
                            navigation.binding.wrappedValue = path
                        }
                    }
                    .accessibilityIdentifier("bsvPublicFundingSubmit")
                }
            } else {
                BsvMultisigNotice(text: Localized.multisigFundingExpired, tone: .warning)
            }
            Button(Localized.Common.cancel) { model.cancelActions(); dismiss() }.buttonStyle(.wowPaySecondary())
        }
    }
}
