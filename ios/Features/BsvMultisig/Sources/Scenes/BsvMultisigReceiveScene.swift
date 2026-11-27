import Components
import enum BsvHomeCore.BsvHomeBsvMultisigError
import Localization
import Style
import SwiftUI

struct BsvMultisigReceiveScene: View {
    private struct Scope: Hashable {
        let configurationId: String?
        let canTransact: Bool
    }

    private struct PaymentInput: Hashable {
        let amount: String
        let scope: Scope
    }

    @Bindable var model: BsvMultisigViewModel
    @Environment(\.displayScale) private var displayScale
    let walletId: String
    @State private var amount = ""
    @State private var configuration: String?
    @State private var request: String?
    @State private var qrImage: UIImage?
    @State private var isLoading = true
    @State private var codeError: String?
    @State private var codeRevision: UInt = 0
    @State private var isSharingPaymentCode = false

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigReceiveTitle, subtitle: model.wallet(walletId)?.name ?? "") {
            if let wallet = model.wallet(walletId), wallet.canTransact {
                NavigationLink(Localized.multisigFunding, value: BsvMultisigRoute.funding(walletId)).buttonStyle(.wowPay())
                    .disabled(model.isWorking)
                VStack(alignment: .leading, spacing: Spacing.space12) {
                    BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                    BsvMultisigValueRow(title: Localized.multisigThreshold, value: wallet.rule)
                    Text(Localized.multisigPaymentScanHint).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                    TextField(Localized.multisigAmount, text: $amount)
                        .keyboardType(.decimalPad).wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                        .accessibilityIdentifier("bsvMultisigReceiveAmount")
                    if isLoading {
                        LoadingView().frame(maxWidth: .infinity)
                    } else if let qrImage {
                        Image(uiImage: qrImage).resizable().interpolation(.none).scaledToFit()
                            .frame(width: Sizing.wowPay.multisigPaymentQR, height: Sizing.wowPay.multisigPaymentQR)
                            .padding(Spacing.space12)
                            .background(Colors.whiteSolid, in: RoundedRectangle(cornerRadius: Sizing.wowPay.inputCorner))
                            .frame(maxWidth: .infinity)
                            .accessibilityLabel(Localized.multisigPaymentTitle)
                            .accessibilityIdentifier("bsvMultisigPaymentQR")
                        Button(Localized.multisigPaymentShare) { isSharingPaymentCode = true }.buttonStyle(.wowPaySecondary())
                    } else if let codeError {
                        BsvMultisigNotice(text: codeError, tone: .error)
                    } else if request != nil {
                        BsvMultisigNotice(text: Localized.multisigQrTooLarge, tone: .warning)
                    }
                    if let configuration {
                        BsvMultisigValueRow(title: Localized.multisigConfigurationId, value: wallet.configurationId ?? Localized.Common.notAvailable)
                        Button(Localized.Common.copy) { model.copy(configuration) }.buttonStyle(.wowPaySecondary())
                    }
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                BsvMultisigPaymailCard(model: model, walletId: walletId)
                if configuration != nil {
                    Button(Localized.multisigBackup) { Task { await model.exportBackup(walletId) } }.buttonStyle(.wowPay())
                }
            } else {
                BsvMultisigNotice(text: model.wallet(walletId)?.configurationStatusTitle ?? Localized.multisigErrorMissingRecord, tone: .warning)
                NavigationLink(Localized.multisigMembersRules, value: BsvMultisigRoute.members(walletId)).buttonStyle(.wowPaySecondary())
            }
        }
        .accessibilityIdentifier("bsvMultisigReceive")
        .bsvMultisigRefresh { await model.refreshWallet(walletId) }
        .task(id: scope) {
            configuration = nil
            guard model.wallet(walletId)?.canTransact == true else { return }
            let current = await model.receiveRequest(walletId)
            guard !Task.isCancelled, model.wallet(walletId)?.canTransact == true else { return }
            configuration = current
            await model.loadPaymail(walletId)
        }
        .task(id: PaymentInput(amount: amount, scope: scope)) { await reloadCode() }
        .sheet(isPresented: $isSharingPaymentCode) {
            if let qrImage {
                ShareSheet(activityItems: [qrImage])
            }
        }
    }

    private var scope: Scope {
        Scope(configurationId: model.wallet(walletId)?.configurationId, canTransact: model.wallet(walletId)?.canTransact == true)
    }

    private func reloadCode() async {
        codeRevision &+= 1
        let ticket = codeRevision
        request = nil
        qrImage = nil
        codeError = nil
        isLoading = true
        defer {
            if codeRevision == ticket {
                isLoading = false
            }
        }
        guard model.wallet(walletId)?.canTransact == true else { return }
        do {
            let payload = try await model.paymentPayload(walletId, amount: amount.trimmingCharacters(in: .whitespacesAndNewlines))
            guard !Task.isCancelled, codeRevision == ticket else { return }
            request = payload
            guard payload.utf8.count <= model.qrSizeLimit else { return }
            let pixelSize = Sizing.wowPay.multisigPaymentQR * displayScale
            let image = await QRCodeGenerator().generate(from: payload, size: CGSize(width: pixelSize, height: pixelSize), correctionLevel: .medium)
            guard !Task.isCancelled, codeRevision == ticket else { return }
            qrImage = image
        } catch {
            guard !Task.isCancelled, codeRevision == ticket else { return }
            codeError = (error as? BsvHomeBsvMultisigError)?.localizedMessage ?? Localized.multisigPaymentInvalid
        }
    }
}
