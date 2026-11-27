import Components
import struct BsvHomeCore.BsvHomeBsvMultisigInvitation
import Localization
import Style
import SwiftUI
import UIKit

struct BsvMultisigInviteScene: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.displayScale) private var displayScale
    @Bindable var model: BsvMultisigViewModel
    let walletId: String
    var completion: BsvMultisigInvitationCompletion = .openWallet
    @State private var invitation: BsvHomeBsvMultisigInvitation?
    @State private var invitationPayload: String?
    @State private var qrImage: UIImage?
    @State private var isGeneratingQRCode = false

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigInvite, subtitle: model.wallet(walletId)?.name ?? "") {
            if let wallet = model.wallet(walletId) {
                VStack(alignment: .leading, spacing: Spacing.small) {
                    BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                    BsvMultisigValueRow(title: Localized.multisigRule, value: wallet.rule)
                    ForEach(wallet.members, id: \.slot) { BsvMultisigMemberRow(member: $0, onCopy: model.copy) }
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                BsvMultisigNotice(text: Localized.multisigInviteCodeNotice)
                if wallet.canInvite {
                    if let invitation {
                        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                            Text(invitation.token).font(.system(.body, design: .monospaced)).textSelection(.enabled)
                            if isGeneratingQRCode {
                                LoadingView().frame(maxWidth: .infinity)
                            } else if let qrImage {
                                Image(uiImage: qrImage).resizable().interpolation(.none).scaledToFit()
                                    .frame(width: Sizing.wowPay.receiveQR, height: Sizing.wowPay.receiveQR)
                                    .padding(Spacing.space12)
                                    .background(Colors.whiteSolid, in: RoundedRectangle(cornerRadius: Sizing.wowPay.inputCorner))
                                    .accessibilityLabel(Localized.multisigInvitation)
                                    .frame(maxWidth: .infinity)
                            } else if invitationPayload != nil {
                                BsvMultisigNotice(text: Localized.Errors.errorOccurred, tone: .warning)
                            }
                            BsvMultisigValueRow(title: Localized.multisigExpiresAt, value: model.date(invitation.expiresAt))
                            HStack(spacing: Spacing.wowPay.grid) {
                                Button(Localized.multisigCopyInvite) {
                                    if let invitationPayload {
                                        model.copy(invitationPayload)
                                    }
                                }.buttonStyle(.wowPaySecondary())
                                Button(Localized.Common.share) {
                                    if let invitationPayload {
                                        model.share(invitationPayload, filename: "bsvhome-multisig-invite.txt")
                                    }
                                }.buttonStyle(.wowPaySecondary())
                            }
                            .disabled(invitationPayload == nil)
                        }
                        .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                    }
                    StateButton(text: invitation == nil ? Localized.multisigGenerateInvite : Localized.multisigRegenerateInvite, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : .normal)) {
                        Task { invitation = await model.invitation(walletId) }
                    }
                }
                NavigationLink(Localized.multisigMembersRules, value: BsvMultisigRoute.members(walletId)).buttonStyle(.wowPaySecondary())
                switch completion {
                case .returnToWallet:
                    Button(Localized.multisigEnterWallet) { dismiss() }.buttonStyle(.wowPay())
                case .openWallet:
                    NavigationLink(Localized.multisigEnterWallet, value: BsvMultisigRoute.wallet(walletId)).buttonStyle(.wowPay())
                }
            }
        }
        .bsvMultisigRefresh { await model.refreshWallet(walletId) }
        .onChange(of: model.wallet(walletId)?.members) { _, _ in invitation = nil }
        .onChange(of: ObjectIdentifier(model)) { _, _ in invitation = nil }
        .task(id: invitation?.token) {
            invitationPayload = nil
            qrImage = nil
            isGeneratingQRCode = false
            guard let token = invitation?.token, let payload = model.invitationQrPayload(token) else { return }
            isGeneratingQRCode = true
            defer {
                if invitation?.token == token {
                    isGeneratingQRCode = false
                }
            }
            invitationPayload = payload
            let pixelSize = Sizing.wowPay.receiveQR * displayScale
            let generated = await QRCodeGenerator().generate(from: payload, size: CGSize(width: pixelSize, height: pixelSize))
            guard !Task.isCancelled, invitation?.token == token else { return }
            qrImage = generated
        }
    }
}
