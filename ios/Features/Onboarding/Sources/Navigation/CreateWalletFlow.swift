// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Primitives

@MainActor
public final class CreateWalletFlow {
    public enum Step: String, Hashable, Codable {
        case acceptTerms
        case securityReminder
        case showSecret
        case verifyPhrase
        case walletProfile
    }

    let model: CreateWalletModel
    private let onNavigate: (Step) -> Void
    var generatedWords: [String]? { model.generatedWords }
    var createdWallet: CreatedWallet? { model.createdWallet }

    public init(model: CreateWalletModel, onNavigate: @escaping (Step) -> Void) {
        self.model = model
        self.onNavigate = onNavigate
    }

    public var initialStep: Step {
        model.isAcceptTermsCompleted ? .securityReminder : .acceptTerms
    }

    public func discard() {
        model.cancel()
    }

    func acceptTerms() {
        onNavigate(.securityReminder)
    }

    func revealSecretPhrase() {
        do {
            try model.revealSecretPhrase()
            onNavigate(.showSecret)
        } catch {
            model.isPresentingAlertMessage = AlertMessage(title: Localized.Errors.createWallet(""), error: error)
        }
    }

    func verifySecretPhrase() {
        onNavigate(.verifyPhrase)
    }

    func completeVerification(words: [String]) async throws {
        let completion = try await model.completeCreation(words: words)
        try model.checkCreationIsActive()
        switch completion {
        case let .personal(result):
            if result.hasExistingWallets {
                model.clearGeneratedWords()
                onNavigate(.walletProfile)
            } else {
                try model.activateWallet(wallet: result.wallet)
            }
        case let .dedicatedMultisigIdentity(wallet, onComplete):
            model.finishCreation()
            onComplete(wallet)
        }
    }

    func completeProfile(wallet: Wallet) {
        model.setupWalletComplete(wallet: wallet)
    }
}
