import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigCreateReviewScene: View {
    @Environment(BsvMultisigNavigation.self) private var navigation
    @Environment(\.scenePhase) private var scenePhase
    @Bindable var model: BsvMultisigViewModel
    let review: BsvMultisigCreationReview
    @State private var operationTask: Task<Void, Never>?
    @State private var operationRevision: UInt = 0

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigConfirmCreation, subtitle: "") {
            VStack(alignment: .leading, spacing: Spacing.small) {
                BsvMultisigValueRow(title: Localized.Wallet.name, value: review.name)
                BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                BsvMultisigValueRow(title: Localized.multisigThreshold, value: "\(review.threshold)/\(review.memberNames.count)")
                BsvMultisigValueRow(title: Localized.multisigSigningWallet, value: model.signerName(review.identity))
                BsvMultisigValueRow(title: Localized.Common.address, value: review.identity.address, onCopy: { model.copy(review.identity.address) })
                BsvMultisigValueRow(title: Localized.multisigPublicKey, value: review.identity.publicKey, onCopy: { model.copy(review.identity.publicKey) })
                ForEach(review.memberNames.indices, id: \.self) { index in
                    BsvMultisigValueRow(title: Localized.multisigMemberName(index + 1), value: review.memberNames[index])
                }
            }
            .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
            if let resolvedWalletId {
                Button(Localized.multisigContinueInvite) { navigation.replaceLast(.invite(resolvedWalletId)) }
                    .buttonStyle(.wowPay())
            } else if isUncertain {
                BsvMultisigNotice(text: Localized.multisigCreationUncertainHint, tone: .warning)
                StateButton(text: Localized.multisigCheckCreation, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : .normal)) {
                    startOperation { await model.reconcileCreation(review) }
                }
                if let candidates = model.creationCandidates {
                    if candidates.isEmpty {
                        BsvMultisigNotice(text: Localized.multisigCreationNoMatches)
                    } else {
                        Text(Localized.multisigCreationCandidates).font(.headline)
                        BsvMultisigNotice(text: Localized.multisigCreationCandidateHint)
                        ForEach(candidates, id: \.id) { wallet in
                            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                                Text(wallet.name).font(.headline)
                                Text(wallet.rule).font(.subheadline)
                                BsvMultisigValueRow(title: Localized.multisigWalletId, value: wallet.id, onCopy: { model.copy(wallet.id) })
                                ForEach(wallet.members, id: \.slot) { BsvMultisigMemberRow(member: $0, onCopy: model.copy) }
                                StateButton(text: Localized.multisigAcceptCreatedWallet, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : .normal)) {
                                    startOperation {
                                        if let id = await model.acceptCreation(review, walletId: wallet.id), !Task.isCancelled {
                                            navigation.replaceLast(.invite(id))
                                        }
                                    }
                                }
                            }
                            .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                        }
                    }
                }
            } else {
                BsvMultisigNotice(text: Localized.multisigCreateConfirmationNotice)
                StateButton(text: Localized.multisigCreateInvite, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : .normal)) {
                    startOperation {
                        if let id = await model.submitCreation(review), !Task.isCancelled {
                            navigation.replaceLast(.invite(id))
                        }
                    }
                }
                .accessibilityIdentifier("bsvMultisigCreateSubmit")
            }
        }
        .accessibilityIdentifier("bsvMultisigCreateReviewPage")
        .onDisappear(perform: cancelOperation)
        .onChange(of: ObjectIdentifier(model)) { _, _ in cancelOperation() }
        .onChange(of: scenePhase) {
            _, phase in if phase == .background {
                cancelOperation()
            }
        }
    }

    private var isUncertain: Bool {
        if model.uncertainCreationDraftId == review.draft.id {
            return true
        }
        if case .submitted = review.draft.status {
            return true
        }
        return false
    }

    private var resolvedWalletId: String? {
        if case let .resolved(walletId) = review.draft.status {
            return walletId
        }
        return nil
    }

    private func startOperation(_ action: @escaping @MainActor () async -> Void) {
        guard operationTask == nil else { return }
        operationRevision &+= 1
        let currentRevision = operationRevision
        operationTask = Task {
            defer {
                if currentRevision == operationRevision {
                    operationTask = nil
                }
            }
            guard !Task.isCancelled else { return }
            await action()
        }
    }

    private func cancelOperation() {
        operationRevision &+= 1
        operationTask?.cancel()
        operationTask = nil
    }
}
