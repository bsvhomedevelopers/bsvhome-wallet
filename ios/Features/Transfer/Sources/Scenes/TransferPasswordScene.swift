// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Style
import SwiftUI

public struct TransferPasswordScene: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @Bindable private var model: TransferPasswordViewModel

    public init(model: TransferPasswordViewModel) {
        self.model = model
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                Text(model.walletName)
                    .font(.wowPay.cardTitle)
                    .foregroundStyle(Colors.WowPay.heading)
                switch model.state {
                case .noData:
                    LoadingView()
                        .accessibilityIdentifier("transferPasswordNotLoaded")
                case .loading:
                    LoadingView()
                        .accessibilityIdentifier("transferPasswordLoading")
                case let .data(snapshot):
                    Text(model.hint)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                    Text(snapshot.address)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                        .textSelection(.enabled)
                    passwordFields
                    if let message = model.errorMessage {
                        Text(message)
                            .font(.wowPay.caption)
                            .foregroundStyle(Colors.red)
                            .accessibilityIdentifier("transferPasswordError")
                    }
                    StateButton(
                        text: model.buttonTitle,
                        type: .wowPayPrimary(model.isWorking ? .loading() : model.canSubmit ? .normal : .disabled),
                        action: model.submit,
                    )
                    .accessibilityIdentifier("transferPasswordSubmit")
                case let .error(error):
                    StateEmptyView(title: model.errorDescription(error)) {
                        Button(Localized.Common.tryAgain, action: model.reloadStatus)
                            .buttonStyle(.wowPaySecondary())
                    }
                    .accessibilityIdentifier("transferPasswordLoadError")
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .wowPayCard()
            .padding(Spacing.wowPay.page)
        }
        .scrollDismissesKeyboard(.interactively)
        .wowPayBackground(.solid)
        .navigationTitle(model.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(Localized.Common.cancel) { model.cancel(); dismiss() }
                    .accessibilityIdentifier("transferPasswordCancel")
            }
        }
        .onAppear(perform: model.startLoading)
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                model.cancel(); dismiss()
            }
        }
        .onChange(of: model.isCancelled) { _, cancelled in
            if cancelled {
                dismiss()
            }
        }
        .onChange(of: model.isComplete) { _, complete in
            if complete, model.mode == .management {
                dismiss()
            }
        }
        .privacySensitive()
        .protectFromScreenRecording()
    }

    private var passwordFields: some View {
        VStack(spacing: Spacing.space12) {
            if model.showsCurrentPassword {
                SecureField(model.currentPasswordTitle, text: $model.currentPassword)
                    .textContentType(.password)
                    .accessibilityIdentifier("transferPasswordCurrent")
            }
            if model.showsNewPassword {
                SecureField(Localized.wowpayTransferPasswordNew, text: $model.newPassword)
                    .textContentType(.newPassword)
                    .accessibilityIdentifier("transferPasswordNew")
                SecureField(Localized.wowpayTransferPasswordConfirmation, text: $model.confirmation)
                    .textContentType(.newPassword)
                    .accessibilityIdentifier("transferPasswordConfirmation")
            }
        }
        .textFieldStyle(.roundedBorder)
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .disabled(model.isWorking)
        .submitLabel(.go)
        .onSubmit(model.submit)
    }
}
