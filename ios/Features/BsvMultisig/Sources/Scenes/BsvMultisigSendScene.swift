import Components
import Foundation
import Localization
import Style
import SwiftUI

struct BsvMultisigSendScene: View {
    @Environment(NavigationPathState.self) private var navigationOwner: NavigationPathState?
    @Environment(\.bsvMultisigRecipientScanner) private var onScanRecipient
    @Environment(\.scenePhase) private var scenePhase
    @Environment(BsvMultisigNavigation.self) private var navigation
    @Bindable var model: BsvMultisigViewModel
    let walletId: String
    let initialRecipientCode: String?
    @State private var hasAppliedInitialRecipient = false
    @State private var scannerTicket: UUID?
    @State private var destination = ""
    @State private var amount = ""
    @State private var memo = ""

    init(model: BsvMultisigViewModel, walletId: String, initialRecipientCode: String? = nil) {
        self.model = model
        self.walletId = walletId
        self.initialRecipientCode = initialRecipientCode
    }

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigSendTitle, subtitle: model.wallet(walletId)?.name ?? "") {
            if let wallet = model.wallet(walletId) {
                VStack(alignment: .leading, spacing: Spacing.wowPay.multisigField) {
                    BsvMultisigValueRow(title: Localized.multisigSharedAssets, value: model.amount(wallet.balanceSatoshis, symbol: true))
                    Text(Localized.Common.address).font(.wowPay.multisigLabel)
                    TextField(Localized.multisigP2pkhPlaceholder, text: $destination)
                        .textInputAutocapitalization(.never).autocorrectionDisabled().font(.body).wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                    if let onScanRecipient {
                        Button(Localized.wowpaySendScanQr) {
                            let ticket = UUID()
                            let identity = ObjectIdentifier(model)
                            scannerTicket = ticket
                            onScanRecipient { value in
                                guard scannerTicket == ticket, ObjectIdentifier(model) == identity, !model.isCancelled, !model.isWorking else { return }
                                scannerTicket = nil
                                applyRecipient(value)
                            }
                        }
                        .buttonStyle(.wowPaySecondary()).disabled(model.isWorking)
                        .accessibilityIdentifier("bsvMultisigScanRecipient")
                    }
                    Text(Localized.wowpayAmount).font(.wowPay.multisigLabel)
                    TextField("0.00000000", text: $amount).keyboardType(.decimalPad).font(.body).wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                    Text(Localized.multisigMemo).font(.wowPay.multisigLabel)
                    TextField(Localized.multisigMemoPlaceholder, text: $memo, axis: .vertical)
                        .lineLimit(3 ... 5).font(.body).wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                        .onChange(of: memo) { _, value in memo = String(value.prefix(100)) }
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                VStack(alignment: .leading, spacing: Spacing.small) {
                    BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                    BsvMultisigValueRow(title: Localized.multisigRule, value: wallet.rule)
                    Text(Localized.wowpaySwapFeeAtConfirmation).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                BsvMultisigNotice(text: Localized.multisigProposalNotice)
                StateButton(text: Localized.multisigCreateProposal, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : wallet.canTransact && !wallet.balanceIsStale && !destination.isEmpty && !amount.isEmpty ? .normal : .disabled)) {
                    let reviewedDestination = destination
                    let reviewedAmount = amount
                    let reviewedMemo = memo
                    model.createProposal(walletId: walletId, destination: reviewedDestination, amount: reviewedAmount, memo: reviewedMemo, navigation: navigationOwner) {
                        navigation.append(.proposal($0))
                    }
                }
            }
        }
        .onAppear {
            guard !hasAppliedInitialRecipient else { return }
            hasAppliedInitialRecipient = true
            if let initialRecipientCode {
                applyRecipient(initialRecipientCode)
            }
        }
        .onChange(of: ObjectIdentifier(model)) { _, _ in scannerTicket = nil }
        .onChange(of: walletId) { _, _ in scannerTicket = nil }
        .onChange(of: destination) { _, _ in model.cancelActions() }
        .onChange(of: amount) { _, _ in model.cancelActions() }
        .onChange(of: memo) { _, _ in model.cancelActions() }
        .onChange(of: scenePhase) {
            _, phase in if phase == .background {
                scannerTicket = nil
                model.cancelActions()
            }
        }
        .bsvMultisigRefresh { await model.refreshWallet(walletId) }
    }

    private func applyRecipient(_ code: String) {
        model.scanRecipient(code) { payment in
            destination = payment.recipient.address
            if let value = payment.amount {
                amount = model.amounts.input(value)
            }
        }
    }
}
