import Components
import Localization
import PrimitivesComponents
import Style
import SwiftUI

struct BsvHdImportReviewScene: View {
    @Bindable var model: ImportWalletSceneViewModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                if let preview = model.preview {
                    if preview.mode == .recovery {
                        Text(Localized.bsvHdOnlineSecretHint)
                            .font(.wowPay.caption)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                            .wowPayCard(surface: Colors.WowPay.pale)
                    }
                    BsvHdImportReview(accountPath: preview.accountPath, firstAddress: preview.account.address, isConfirmed: $model.isHdConfirmed)
                        .disabled(model.isBusy)
                        .wowPayCard()
                    StateButton(
                        text: Localized.Wallet.Import.action,
                        type: .wowPayPrimary(model.isHdConfirmed ? model.buttonState : .disabled),
                        action: { Task { await model.confirmHdImport() } },
                    )
                    .accessibilityIdentifier("bsvHdConfirmImport")
                } else if model.isBusy {
                    ProgressView()
                }
            }
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .navigationTitle(Localized.bsvHdPreviewTitle)
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(model.isBusy)
        .interactiveDismissDisabled(model.isBusy)
        .modifier(ImportWalletProtection(model: model))
        .onDisappear { model.clearSensitiveState() }
    }
}
