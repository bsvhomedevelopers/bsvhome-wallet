import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigFundingScene: View {
    @Environment(NavigationPathState.self) private var navigationOwner: NavigationPathState?
    @Environment(\.scenePhase) private var scenePhase
    @Environment(BsvMultisigNavigation.self) private var navigation
    @Bindable var model: BsvMultisigViewModel
    let walletId: String
    @State private var payerWalletId = ""
    @State private var amount = ""
    @State private var hasLoaded = false

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigFunding, subtitle: model.wallet(walletId)?.name ?? "") {
            if model.fundingResult?.status == .pending {
                BsvMultisigNotice(text: Localized.multisigFundingPendingNotice, tone: .warning)
                NavigationLink(Localized.multisigFundingResult, value: BsvMultisigRoute.fundingResult(walletId)).buttonStyle(.wowPay())
            } else if hasLoaded {
                VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                    BsvMultisigFundingPayerPicker(payers: model.fundingPayers, selectedWalletId: payerWalletId) { payerWalletId = $0.walletId }
                    Text(Localized.multisigFundingPayerNotice).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                    BsvMultisigValueRow(title: Localized.multisigFundingTarget, value: model.wallet(walletId)?.name ?? "—")
                    BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                    Text(Localized.wowpayAmount).font(.wowPay.multisigLabel)
                    TextField("0.00000000", text: $amount).keyboardType(.decimalPad).font(.body).wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                    Text(Localized.multisigFundingPayerNotice).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                StateButton(text: Localized.multisigFundingReview, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : payerWalletId.isEmpty || amount.isEmpty || model.wallet(walletId)?.canTransact != true ? .disabled : .normal)) {
                    let payer = payerWalletId
                    let reviewedAmount = amount
                    model.prepareFunding(walletId: walletId, payerWalletId: payer, amount: reviewedAmount, navigation: navigationOwner) {
                        navigation.append(.fundingReview($0.id))
                    }
                }
            } else if model.isWorking {
                LoadingView()
            } else {
                Button(Localized.Common.tryAgain) { Task { await load() } }.buttonStyle(.wowPaySecondary())
            }
        }
        .task { await load() }
        .onChange(of: payerWalletId) { _, _ in model.cancelActions() }
        .onChange(of: amount) { _, _ in model.cancelActions() }
        .onChange(of: scenePhase) {
            _, phase in if phase == .background {
                model.cancelActions()
            }
        }
    }

    private func load() async {
        hasLoaded = await model.loadFunding(walletId)
        if !model.fundingPayers.contains(where: { $0.walletId == payerWalletId }) {
            payerWalletId = model.fundingPayers.first?.walletId ?? ""
        }
    }
}
