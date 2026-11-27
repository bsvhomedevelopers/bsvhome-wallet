import Components
import struct BsvHomeCore.BsvHomeBsvMultisigInvitePreview
import Localization
import Style
import SwiftUI

struct BsvMultisigJoinReviewScene: View {
    @Environment(BsvMultisigNavigation.self) private var navigation
    @Environment(\.scenePhase) private var scenePhase
    @Bindable var model: BsvMultisigViewModel
    let preview: BsvHomeBsvMultisigInvitePreview
    @State private var hasReviewed = false

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigConfirmJoin, subtitle: preview.wallet.name) {
            VStack(alignment: .leading, spacing: Spacing.small) {
                BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                BsvMultisigValueRow(title: Localized.multisigRule, value: preview.wallet.rule)
                BsvMultisigValueRow(title: Localized.multisigSigningWallet, value: model.signerName(preview.signingIdentity))
                BsvMultisigValueRow(title: Localized.Common.address, value: preview.signingIdentity.address, onCopy: { model.copy(preview.signingIdentity.address) })
                BsvMultisigValueRow(title: Localized.multisigPublicKey, value: preview.signingIdentity.publicKey, onCopy: { model.copy(preview.signingIdentity.publicKey) })
                BsvMultisigValueRow(title: Localized.multisigExpiresAt, value: model.date(preview.expiresAt))
                ForEach(preview.wallet.members, id: \.slot) { BsvMultisigMemberRow(member: $0, onCopy: model.copy) }
            }
            .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
            BsvMultisigNotice(text: Localized.multisigConfigurationNotice)
            Toggle(Localized.multisigJoinCheck, isOn: $hasReviewed).font(.caption).tint(Colors.WowPay.brand)
            StateButton(text: Localized.multisigConfirmJoin, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : hasReviewed ? .normal : .disabled)) {
                Task {
                    if let id = await model.join(preview) {
                        navigation.append(.members(id))
                    } else {
                        hasReviewed = false
                    }
                }
            }
        }
        .onChange(of: scenePhase) { _, _ in hasReviewed = false }
    }
}
