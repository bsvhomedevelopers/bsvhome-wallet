import Components
import enum BsvHomeCore.BsvHomeBsvMultisigError
import Localization
import Style
import SwiftUI

struct BsvMultisigFundingResultScene: View {
    @Environment(NavigationPathState.self) private var navigationOwner: NavigationPathState?
    @Environment(\.scenePhase) private var scenePhase
    @Bindable var model: BsvMultisigViewModel
    let walletId: String

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigFundingResult, subtitle: model.wallet(walletId)?.name ?? "") {
            if let result = model.fundingResult, result.memberWalletId == walletId {
                BsvMultisigFundingResultContent(result: result, targetName: result.targetName(memberName: { model.wallet($0)?.name }), amounts: model.amounts) {
                    model.copy(result.transactionId)
                }
                if let code = result.errorCode {
                    BsvMultisigNotice(text: BsvHomeBsvMultisigError.Api(code: code).localizedMessage, tone: .warning)
                }
                if result.status == .pending {
                    BsvMultisigNotice(text: Localized.multisigFundingPendingNotice, tone: .warning)
                    StateButton(text: Localized.multisigFundingRetry, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : .normal)) {
                        model.retryFunding(walletId, navigation: navigationOwner)
                    }
                } else {
                    BsvMultisigNotice(text: Localized.multisigFundingBroadcastNotice)
                    if model.fundingSyncFailed {
                        BsvMultisigNotice(text: Localized.multisigFundingSyncFailed, tone: .warning)
                        Button(Localized.multisigRefresh) { Task { await model.retryFundingSync() } }.buttonStyle(.wowPaySecondary()).disabled(model.isWorking)
                    }
                    NavigationLink(Localized.multisigEnterWallet, value: BsvMultisigRoute.wallet(walletId)).buttonStyle(.wowPay())
                }
            } else {
                Button(Localized.Common.tryAgain) { Task { await model.loadFundingResult(walletId) } }.buttonStyle(.wowPaySecondary())
            }
        }
        .onChange(of: scenePhase) {
            _, phase in if phase == .background {
                model.cancelActions()
            }
        }
        .task {
            if model.fundingResult?.memberWalletId != walletId {
                await model.loadFundingResult(walletId)
            }
        }
    }
}
