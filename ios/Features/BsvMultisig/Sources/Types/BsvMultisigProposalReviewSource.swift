import struct BsvHomeCore.BsvHomeBsvMultisigProposal

struct BsvMultisigProposalReviewSource: Hashable {
    let model: ObjectIdentifier
    let proposal: BsvHomeBsvMultisigProposal?

    @MainActor
    init(model: BsvMultisigViewModel, proposalId: String) {
        self.model = ObjectIdentifier(model)
        proposal = model.proposal(proposalId)
    }
}
