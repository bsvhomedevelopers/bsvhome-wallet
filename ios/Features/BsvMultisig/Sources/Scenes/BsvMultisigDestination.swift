import Components
import Localization
import SwiftUI

public struct BsvMultisigDestination: View {
    let model: BsvMultisigViewModel
    let route: BsvMultisigRoute
    let initialInvitation: String?
    let initialRecipientCode: String?

    public init(model: BsvMultisigViewModel, route: BsvMultisigRoute, initialInvitation: String? = nil, initialRecipientCode: String? = nil) {
        self.model = model
        self.route = route
        self.initialInvitation = initialInvitation
        self.initialRecipientCode = initialRecipientCode
    }

    public var body: some View {
        switch route {
        case .list:
            BsvMultisigPage(model: model, title: Localized.multisigTitle, subtitle: "") {
                BsvMultisigListView(model: model, walletId: nil)
            }
        case .create: BsvMultisigCreateScene(model: model)
        case let .createReview(review): BsvMultisigCreateReviewScene(model: model, review: review)
        case .join: BsvMultisigJoinScene(model: model, initialInvitation: initialInvitation)
        case let .joinReview(preview): BsvMultisigJoinReviewScene(model: model, preview: preview)
        case let .wallet(id): BsvMultisigWalletScene(model: model, walletId: id)
        case let .invite(id, completion): BsvMultisigInviteScene(model: model, walletId: id, completion: completion)
        case let .members(id): BsvMultisigMembersScene(model: model, walletId: id)
        case let .backup(id): BsvMultisigBackupScene(model: model, walletId: id)
        case let .receive(id): BsvMultisigReceiveScene(model: model, walletId: id)
        case let .send(id): BsvMultisigSendScene(model: model, walletId: id, initialRecipientCode: initialRecipientCode)
        case let .proposal(id): BsvMultisigProposalScene(model: model, proposalId: id)
        case let .review(id, purpose): BsvMultisigReviewScene(model: model, proposalId: id, purpose: purpose)
        case let .configuration(id): BsvMultisigConfigurationScene(model: model, walletId: id)
        case let .history(id): BsvMultisigHistoryScene(model: model, walletId: id)
        case let .transaction(walletId, transactionId): BsvMultisigTransactionScene(model: model, walletId: walletId, transactionId: transactionId)
        case let .paymail(id): BsvMultisigPaymailScene(model: model, walletId: id)
        case let .settings(id): BsvMultisigSettingsScene(model: model, walletId: id)
        case let .funding(id): BsvMultisigFundingScene(model: model, walletId: id)
        case let .fundingReview(id): BsvMultisigFundingReviewScene(model: model, preparationId: id)
        case let .fundingResult(id): BsvMultisigFundingResultScene(model: model, walletId: id)
        case let .pending(id):
            BsvMultisigPage(model: model, title: Localized.multisigPending, subtitle: model.wallet(id)?.name ?? "") {
                BsvMultisigListView(model: model, walletId: id)
            }
        }
    }
}
