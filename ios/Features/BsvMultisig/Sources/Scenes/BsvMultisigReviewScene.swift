import Components
import struct BsvHomeCore.BsvHomeBsvMultisigProposalReview
import Localization
import Style
import SwiftUI

struct BsvMultisigReviewScene: View {
    @Environment(NavigationPathState.self) private var navigationOwner: NavigationPathState?
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @Bindable var model: BsvMultisigViewModel
    let proposalId: String
    let purpose: BsvMultisigReviewPurpose
    @State private var hasReviewed = false
    @State private var review: BsvHomeBsvMultisigProposalReview?

    private var title: String { purpose == .sign ? Localized.multisigReviewTitle : Localized.multisigConfirmBroadcast }
    private var canSubmit: Bool {
        review?.canSubmit(purpose: purpose, currentProposal: model.proposal(proposalId), hasReviewed: hasReviewed) ?? false
    }

    var body: some View {
        BsvMultisigPage(model: model, title: title, subtitle: Localized.multisigReviewSubtitle) {
            if let review {
                BsvMultisigProposalSummary(model: model, proposal: review.proposal, reviewFields: review.fields)
                BsvMultisigNotice(text: Localized.multisigReviewNotice, tone: .warning)
                Toggle(Localized.multisigReviewCheck, isOn: $hasReviewed).font(.caption).tint(Colors.WowPay.brand)
                StateButton(text: purpose == .sign ? Localized.multisigAuthenticateSign : Localized.multisigBroadcast, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : canSubmit ? .normal : .disabled)) {
                    hasReviewed = false
                    if purpose == .sign {
                        model.sign(review, navigation: navigationOwner) { _ in dismiss() }
                    } else {
                        model.broadcast(review, navigation: navigationOwner) { _ in dismiss() }
                    }
                }
            }
        }
        .task(id: BsvMultisigProposalReviewSource(model: model, proposalId: proposalId)) {
            hasReviewed = false
            review = await model.reviewSnapshot(proposalId)
        }
        .bsvMultisigRefresh { await model.refreshProposal(proposalId) }
        .onChange(of: scenePhase) { _, phase in
            hasReviewed = false
            if phase == .background {
                model.cancelActions()
            }
        }
        .onChange(of: model.actionRevision) { _, _ in hasReviewed = false }
    }
}
