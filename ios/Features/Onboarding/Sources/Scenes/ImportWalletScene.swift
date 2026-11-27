// Copyright (c). Gem Wallet. All rights reserved.

import Components
import InfoSheet
import Localization
import Primitives
import PrimitivesComponents
import QRScanner
import Style
import SwiftUI

struct ImportWalletScene: View {
    @FocusState private var isInputFocused: Bool
    @State private var model: ImportWalletSceneViewModel

    init(model: ImportWalletSceneViewModel) {
        _model = State(initialValue: model)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                Text(model.importType.title)
                    .font(.wowPay.onboardingTitle)
                    .foregroundStyle(Colors.WowPay.heading)
                    .frame(maxWidth: .infinity, alignment: .center)

                VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                    if model.showImportTypes {
                        Picker("", selection: $model.importType) {
                            ForEach(model.importTypes, id: \.self) { type in
                                Text(type.title).tag(type)
                                    .accessibilityIdentifier("importWalletKind-\(type)")
                            }
                        }
                        .pickerStyle(.segmented)
                    }
                    HStack(alignment: .top) {
                        WalletImportInputField(
                            placeholder: model.importType.description,
                            isSecret: model.shouldProtectInput,
                            isDisabled: model.isBusy,
                            identifier: "importInputField",
                            focus: $isInputFocused,
                            input: $model.input,
                            isSecretVisible: $model.isSecretVisible,
                        )

                        if let nameRecordViewModel = model.nameRecordViewModel, model.importType == .address {
                            NameRecordView(model: nameRecordViewModel)
                        }
                    }
                    .wowPayInput()

                    HStack(spacing: Spacing.wowPay.grid) {
                        Button(action: model.onPaste) {
                            Label { Text(model.pasteButtonTitle) } icon: { model.pasteButtonImage }
                        }
                        .buttonStyle(.wowPaySecondary())
                        if model.type != .multicoin {
                            Button(action: model.onSelectScanQR) {
                                Label { Text(model.qrButtonTitle) } icon: { model.qrButtonImage }
                            }
                            .buttonStyle(.wowPaySecondary())
                        }
                    }
                }
                .disabled(model.isBusy)
                .wowPayCard()

                if model.showsHdSettings {
                    BsvHdImportSettings(configuration: $model.hdConfiguration, passphrase: $model.passphrase, isSecret: model.shouldProtectInput)
                        .disabled(model.isBusy)
                        .wowPayCard()
                    if model.shouldProtectInput {
                        Text(Localized.bsvHdBsvOnlyHint).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                    }
                }

                if let text = model.footerText {
                    Text(.init(text))
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .wowPayCard(surface: Colors.WowPay.pale)
                }
            }
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .scrollDismissesKeyboard(.interactively)
        .safeAreaInset(edge: .bottom, spacing: .zero) {
            VStack(spacing: Spacing.wowPay.grid) {
                if model.showsWordSuggestions, model.wordsSuggestion.isNotEmpty, isInputFocused {
                    WordSuggestionView(
                        words: model.wordsSuggestion,
                        selectWord: model.onSelectWord,
                    )
                }
                StateButton(
                    text: Localized.Wallet.Import.action,
                    type: .wowPayPrimary(model.isBusy || model.canSubmit ? model.buttonState : .disabled),
                    action: onSelectActionButton,
                )
                .accessibilityIdentifier("submitWalletImport")
            }
            .padding(.horizontal, Spacing.wowPay.page)
            .padding(.top, Spacing.wowPay.grid)
            .padding(.bottom, Spacing.scene.bottom)
            .frame(maxWidth: .scene.button.maxWidth)
            .frame(maxWidth: .infinity)
            .background(Colors.WowPay.background)
        }
        .navigationBarTitle(model.title)
        .navigationBarTitleDisplayMode(.inline)
        .alertSheet($model.isPresentingAlertMessage)
        .sheet(isPresented: $model.isPresentingExistingWalletName.mappedToBool()) {
            InfoSheetScene(
                model: InfoSheetModel(
                    title: model.isPresentingExistingWalletName ?? "",
                    description: Localized.Wallet.Import.alreadyImportedMessage,
                    image: .image(Images.Logo.logo),
                    button: .action(
                        title: Localized.Common.continue,
                        action: model.onSelectExistingWalletContinue,
                    ),
                ),
            )
        }
        .sheet(isPresented: $model.isPresentingScanner) {
            ScanQRCodeNavigationStack(scanType: model.scanType, mode: model.importType == .address && model.chain == .bitcoinSv ? .continuous : .single, hint: model.scanHint, action: { value in Task { await model.onHandleScan(value) } })
        }
        .onChange(of: model.input, model.onChangeInput)
        .onChange(of: model.importType, model.onChangeImportType)
        .taskOnce {
            isInputFocused = true
        }
        .detectScreenshots(docsUrl: model.docsUrl)
        .modifier(ImportWalletProtection(model: model))
        .navigationDestination(isPresented: $model.isPresentingHdReview) { BsvHdImportReviewScene(model: model) }
        .onDisappear {
            if !model.isPresentingHdReview, !model.isPresentingScanner {
                model.clearSensitiveState()
            }
        }
    }

    private func onSelectActionButton() {
        isInputFocused = false
        Task {
            await model.onSelectActionButton()
        }
    }
}
