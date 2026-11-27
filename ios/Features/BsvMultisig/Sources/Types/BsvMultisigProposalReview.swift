import struct BsvHomeCore.BsvHomeBsvMultisigProposal
import struct BsvHomeCore.BsvHomeBsvMultisigProposalReview

extension BsvHomeBsvMultisigProposalReview {
    func canSubmit(purpose: BsvMultisigReviewPurpose, currentProposal: BsvHomeBsvMultisigProposal?, hasReviewed: Bool) -> Bool {
        guard hasReviewed, currentProposal == proposal else { return false }
        return purpose == .sign ? proposal.canSign : proposal.canBroadcast
    }
}
