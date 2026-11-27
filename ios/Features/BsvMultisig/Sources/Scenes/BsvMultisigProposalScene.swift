import Components
import struct BsvHomeCore.BsvHomeBsvMultisigProposalReview
import Localization
import Style
import SwiftUI

struct BsvMultisigProposalScene: View {
    @Environment(NavigationPathState.self) private var navigationOwner: NavigationPathState?
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.dismiss) private var dismiss
    @Bindable var model: BsvMultisigViewModel
    let proposalId: String
    @State private var availableReview: BsvHomeBsvMultisigProposalReview?
    @State private var withdrawReview: BsvHomeBsvMultisigProposalReview?

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigProposal, subtitle: model.proposal(proposalId).flatMap { model.wallet($0.walletId)?.name } ?? "") {
            if let proposal = model.proposal(proposalId), let wallet = model.wallet(proposal.walletId) {
                BsvMultisigProposalSummary(model: model, proposal: proposal)
                VStack(alignment: .leading, spacing: Spacing.small) {
                    HStack {
                        Text(Localized.multisigSigningProgress).font(.headline)
                        Spacer()
                        Text("\(proposal.signedMembers.count)/\(proposal.requiredSignatures)").font(.subheadline.weight(.bold))
                    }
                    BsvMultisigSignatureProgress(signedMembers: proposal.signedMembers.count, requiredSignatures: proposal.requiredSignatures)
                    ForEach(wallet.members, id: \.slot) { BsvMultisigMemberRow(member: $0, signedMembers: proposal.signedMembers) }
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                if proposal.canSign {
                    NavigationLink(Localized.multisigReviewSign, value: BsvMultisigRoute.review(proposalId, purpose: .sign)).buttonStyle(.wowPay())
                }
                if proposal.canBroadcast {
                    NavigationLink(Localized.multisigConfirmBroadcast, value: BsvMultisigRoute.review(proposalId, purpose: .broadcast)).buttonStyle(.wowPay())
                }
                if proposal.canWithdraw {
                    Button(Localized.multisigWithdraw) { withdrawReview = availableReview }.buttonStyle(.wowPaySecondary()).disabled(availableReview == nil || model.isWorking)
                }
                if proposal.state == .stopped || proposal.state == .expired {
                    BsvMultisigNotice(text: Localized.multisigWithdrawNotice, tone: .warning)
                }
                if let transactionId = proposal.transactionId {
                    Button(Localized.Common.copy) { model.copy(transactionId) }.buttonStyle(.wowPaySecondary())
                }
                Button(Localized.multisigRefresh) { Task { await model.refreshProposal(proposalId) } }.buttonStyle(.wowPaySecondary()).disabled(model.isWorking)
            }
        }
        .confirmationDialog(Localized.multisigWithdraw, isPresented: withdrawBinding, titleVisibility: .visible, presenting: withdrawReview) { review in
            if review.proposal.canWithdraw, model.proposal(proposalId) == review.proposal {
                Button(Localized.multisigWithdraw, role: .destructive) {
                    withdrawReview = nil
                    model.withdraw(review, navigation: navigationOwner)
                }
            }
            Button(Localized.Common.cancel, role: .cancel) {}
        } message: { _ in Text(Localized.multisigWithdrawNotice) }
        .onChange(of: scenePhase) { _, phase in
            withdrawReview = nil
            if phase == .background {
                model.cancelActions()
            }
        }
        .task(id: BsvMultisigProposalReviewSource(model: model, proposalId: proposalId)) {
            availableReview = await model.reviewSnapshot(proposalId)
            withdrawReview = nil
        }
        .bsvMultisigRefresh { await model.refreshProposal(proposalId) }
        .onChange(of: model.proposal(proposalId)) { _, proposal in
            if proposal == nil {
                dismiss()
            }
        }
    }

    private var withdrawBinding: Binding<Bool> {
        let presentedId = withdrawReview?.reviewId
        return Binding(get: { withdrawReview != nil }, set: {
            if !$0, withdrawReview?.reviewId == presentedId {
                withdrawReview = nil
            }
        })
    }
}
