// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import PrimitivesComponents
import SwiftUI

public struct CreateWalletFlowScene: View {
    private let flow: CreateWalletFlow
    private let step: CreateWalletFlow.Step
    @State private var model: CreateWalletModel

    public init(flow: CreateWalletFlow, step: CreateWalletFlow.Step) {
        self.flow = flow
        self.step = step
        _model = State(initialValue: flow.model)
    }

    public var body: some View {
        sceneContent
            .toolbar(.visible, for: .navigationBar)
            .navigationBarTitleDisplayMode(.inline)
            .sheet(item: $model.isPresentingSelectImageWallet) { wallet in
                NavigationStack {
                    WalletImageScene(model: model.walletImageModel(wallet: wallet))
                        .toolbarDismissItem(type: .close, placement: .topBarLeading)
                }
            }
            .alertSheet($model.isPresentingAlertMessage)
    }

    @ViewBuilder
    private var sceneContent: some View {
        switch step {
        case .acceptTerms:
            AcceptTermsScene(model: AcceptTermsViewModel(onNext: flow.acceptTerms))
        case .securityReminder:
            SecurityReminderScene(
                model: SecurityReminderViewModel(
                    title: Localized.Wallet.New.title,
                    onNext: flow.revealSecretPhrase,
                ),
            )
        case .showSecret:
            if let words = flow.generatedWords {
                ShowSecretDataScene(
                    model: NewSecretPhraseViewModel(
                        words: words,
                        onCreateWallet: { _ in flow.verifySecretPhrase() },
                    ),
                )
            } else {
                unavailableScene
            }
        case .verifyPhrase:
            if let words = flow.generatedWords {
                VerifyPhraseWalletScene(
                    model: VerifyPhraseViewModel(words: words, onComplete: flow.completeVerification),
                )
            } else {
                unavailableScene
            }
        case .walletProfile:
            if let wallet = flow.createdWallet?.wallet {
                SetupWalletScene(model: model.setupWalletModel(wallet: wallet, onComplete: flow.completeProfile))
                    .navigationBarBackButtonHidden()
                    .interactiveDismissDisabled()
            } else {
                unavailableScene
            }
        }
    }

    private var unavailableScene: some View {
        ContentUnavailableView(Localized.Errors.errorOccurred, systemImage: "exclamationmark.triangle")
    }
}
