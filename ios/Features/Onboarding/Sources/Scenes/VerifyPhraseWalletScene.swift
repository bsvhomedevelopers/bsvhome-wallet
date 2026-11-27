// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import PrimitivesComponents
import Style
import SwiftUI

struct VerifyPhraseWalletScene: View {
    @State private var model: VerifyPhraseViewModel

    init(model: VerifyPhraseViewModel) {
        _model = State(initialValue: model)
    }

    var body: some View {
        ScrollView {
            VStack(spacing: Spacing.wowPay.section) {
                Text(Localized.SecretPhrase.Confirm.QuickTest.title)
                    .font(.wowPay.subtitle)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)

                SecretPhraseGridView(
                    rows: model.rows,
                    highlightIndex: model.wordsIndex,
                    style: .wowPay,
                )
                .wowPayCard(padding: Spacing.space12)

                Grid(horizontalSpacing: Spacing.space8, verticalSpacing: Spacing.wowPay.grid) {
                    ForEach(model.rowsSections, id: \.self) { section in
                        GridRow {
                            ForEach(section) { row in
                                Button {
                                    model.pickWord(index: row)
                                } label: {
                                    Text(row.word)
                                        .font(.wowPay.label)
                                        .fixedSize(horizontal: false, vertical: true)
                                        .frame(minHeight: Sizing.wowPay.minimumTapTarget)
                                }
                                .buttonStyle(.styled(
                                    .wowPaySecondary,
                                    paddingHorizontal: Spacing.space4,
                                    paddingVertical: .zero,
                                    cornerRadius: Sizing.wowPay.inputCorner,
                                    glassEffect: .disabled,
                                ))
                                .accessibilityIdentifier("verifyWalletWord-\(row.index)")
                                .disabled(model.isVerified(index: row))
                                .opacity(model.isVerified(index: row) ? WowPayOpacity.disabled : 1)
                            }
                        }
                    }
                }
            }
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .safeAreaButton {
            StateButton(
                text: Localized.Common.continue,
                type: .wowPayPrimary(model.buttonState),
                action: model.onContinue,
            )
            .accessibilityIdentifier("verifyWalletBackup")
        }
        .navigationTitle(model.title)
        .toolbarTitleDisplayMode(.inline)
        .alertSheet($model.isPresentingAlertMessage)
        .detectScreenshots(docsUrl: model.docsUrl)
        .protectFromScreenRecording()
    }
}
