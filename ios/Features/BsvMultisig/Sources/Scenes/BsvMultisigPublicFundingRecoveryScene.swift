import Components
import Foundation
import Localization
import Style
import SwiftUI

struct BsvMultisigPublicFundingRecoveryScene: View {
    private struct Discard: Equatable {
        let modelId: UUID
        let operationId: String
    }

    @Environment(NavigationPathState.self) private var navigation: NavigationPathState?
    @Bindable var model: BsvMultisigPublicFundingViewModel
    @State private var discard: Discard?

    var body: some View {
        BsvMultisigPublicFundingPage(model: model, title: Localized.multisigFundingResult) {
            switch model.recoveryState {
            case .noData, .loading: LoadingView()
            case .error:
                Button(Localized.Common.tryAgain, action: model.reloadResults).buttonStyle(.wowPaySecondary())
            case let .data(snapshot):
                if snapshot.results.isEmpty, snapshot.unsubmitted.isEmpty {
                    BsvMultisigNotice(text: Localized.Errors.noDataAvailable)
                }
                ForEach(snapshot.results, id: \.operationId) { result in
                    VStack(alignment: .leading, spacing: Spacing.small) {
                        BsvMultisigValueRow(title: Localized.multisigFundingOperation, value: result.operationId)
                        BsvMultisigValueRow(title: Localized.multisigFundingTarget, value: result.targetName(memberName: { _ in nil }))
                        BsvMultisigValueRow(title: Localized.multisigTransactionId, value: result.transactionId)
                        BsvMultisigValueRow(title: Localized.wowpayAmount, value: model.amounts.amount(result.amountSatoshis, symbol: true))
                        Button(Localized.multisigFundingResult) {
                            if model.selectRecovery(result.operationId) {
                                navigation?.binding.wrappedValue.append(BsvMultisigPublicFundingRoute.result(result.operationId))
                            }
                        }
                        .buttonStyle(.wowPaySecondary())
                        .accessibilityIdentifier("bsvPublicFundingOperation-" + result.operationId)
                    }
                    .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                }
                ForEach(snapshot.unsubmitted, id: \.operationId) { saved in
                    VStack(alignment: .leading, spacing: Spacing.small) {
                        BsvMultisigNotice(text: Localized.multisigFundingUnsubmitted, tone: .warning)
                        BsvMultisigValueRow(title: Localized.multisigFundingOperation, value: saved.operationId)
                        BsvMultisigValueRow(title: Localized.multisigFundingTarget, value: saved.target.displayName(memberName: { $0 }))
                        BsvMultisigValueRow(title: Localized.multisigFundingPayer, value: saved.payerWalletName)
                        BsvMultisigValueRow(title: Localized.Common.address, value: saved.senderAddress)
                        BsvMultisigValueRow(title: Localized.wowpayAmount, value: model.amounts.amount(saved.amountSatoshis, symbol: true))
                        BsvMultisigValueRow(title: Localized.Transfer.networkFee, value: model.amounts.amount(saved.feeSatoshis, symbol: true))
                        BsvMultisigValueRow(title: Localized.multisigFundingTotal, value: model.amounts.amount(saved.totalSatoshis, symbol: true))
                        Button(Localized.multisigFundingDiscard) { discard = Discard(modelId: model.id, operationId: saved.operationId) }
                            .buttonStyle(.wowPaySecondary()).disabled(model.readTask != nil || model.isWorking)
                            .accessibilityIdentifier("bsvPublicFundingDiscard-" + saved.operationId)
                    }
                    .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                }
            }
        }
        .onAppear(perform: model.startLoading)
        .confirmationDialog(Localized.multisigFundingDiscard, isPresented: discardBinding) {
            if let value = discard {
                Button(Localized.multisigFundingDiscard, role: .destructive) {
                    guard value == discard, value.modelId == model.id else { return }
                    discard = nil
                    model.discardUnsubmitted(value.operationId)
                }
                Button(Localized.Common.cancel, role: .cancel) {
                    if value == discard {
                        discard = nil
                    }
                }
            }
        } message: {
            Text(Localized.multisigFundingDiscardNotice)
        }
    }

    private var discardBinding: Binding<Bool> {
        let value = discard
        return Binding(get: { value != nil && value == discard }, set: {
            if !$0, value == discard {
                discard = nil
            }
        })
    }
}
