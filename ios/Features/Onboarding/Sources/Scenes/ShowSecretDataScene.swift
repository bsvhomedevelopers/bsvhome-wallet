// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

struct ShowSecretDataScene: View {
    let model: any SecretPhraseViewableModel
    @State private var isPresentingCopyToast = false

    var body: some View {
        ScrollView {
            VStack(spacing: Spacing.wowPay.section) {
                if let calloutViewStyle = model.calloutViewStyle {
                    CalloutView(style: calloutViewStyle)
                }

                SecretDataTypeView(type: model.type, style: .wowPay)
                    .wowPayCard(padding: Spacing.space12)

                Button(action: copy) {
                    Label(Localized.Common.copy, systemImage: SystemImage.copy)
                }
                .buttonStyle(.wowPaySecondary())
            }
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .safeAreaButton(isVisible: model.continueAction != nil) {
            StateButton(
                text: Localized.Common.continue,
                type: .wowPayPrimary(),
                action: continueAction,
            )
            .accessibilityIdentifier("continueWalletBackup")
        }
        .toolbarInfoButton(url: model.docsUrl)
        .navigationTitle(model.title)
        .toolbarTitleDisplayMode(.inline)
        .copyToast(
            model: model.copyModel,
            isPresenting: $isPresentingCopyToast,
        )
        .detectScreenshots(docsUrl: model.docsUrl)
        .protectFromScreenRecording()
    }

    private func copy() {
        isPresentingCopyToast = true
    }

    func continueAction() {
        model.continueAction?()
    }
}
