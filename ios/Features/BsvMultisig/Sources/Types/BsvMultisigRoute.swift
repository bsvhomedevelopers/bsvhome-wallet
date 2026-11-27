import struct BsvHomeCore.BsvHomeBsvMultisigInvitePreview

public enum BsvMultisigRoute: Hashable, Sendable {
    case list
    case create
    case createReview(BsvMultisigCreationReview)
    case join
    case joinReview(BsvHomeBsvMultisigInvitePreview)
    case wallet(String)
    case invite(String, completion: BsvMultisigInvitationCompletion = .openWallet)
    case members(String)
    case backup(String)
    case receive(String)
    case send(String)
    case proposal(String)
    case review(String, purpose: BsvMultisigReviewPurpose)
    case configuration(String)
    case pending(String)
    case history(String)
    case transaction(walletId: String, transactionId: String)
    case paymail(String)
    case settings(String)
    case funding(String)
    case fundingReview(String)
    case fundingResult(String)
}
