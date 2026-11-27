// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import protocol BsvHomeCore.BsvHomeAvatarServiceProtocol
import enum BsvHomeCore.BsvHomeWalletImportResult
import protocol BsvHomeCore.BsvHomeWalletServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import Primitives
import PrimitivesComponents
import SwiftUI

@Observable
@MainActor
public final class CreateWalletModel {
    private enum Outcome { case active, completed, cancelled }

    private let service: any BsvHomeWalletServiceProtocol
    private let preferences: ObservablePreferences
    private let avatarService: any BsvHomeAvatarServiceProtocol
    let onComplete: VoidAction
    let purpose: WalletOnboardingPurpose
    private(set) var generatedWords: [String]?
    private(set) var createdWallet: CreatedWallet?
    private(set) var creationTask: Task<WalletCreationCompletion, any Error>?
    private var outcome: Outcome = .active
    private var revision: UInt = 0

    var isPresentingSelectImageWallet: Wallet?
    var isPresentingAlertMessage: AlertMessage?

    public init(
        service: any BsvHomeWalletServiceProtocol,
        preferences: ObservablePreferences,
        avatarService: any BsvHomeAvatarServiceProtocol,
        onComplete: VoidAction,
        purpose: WalletOnboardingPurpose = .personal,
    ) {
        self.service = service
        self.preferences = preferences
        self.avatarService = avatarService
        self.onComplete = onComplete
        self.purpose = purpose
    }

    public var isAcceptTermsCompleted: Bool {
        preferences.isAcceptTermsCompleted
    }

    func setupWalletModel(wallet: Wallet, onComplete: @escaping (Wallet) -> Void) -> SetupWalletViewModel {
        SetupWalletViewModel(
            wallet: wallet,
            service: service,
            onSelectImage: { [weak self] in self?.presentSelectImage(wallet: $0) },
            onComplete: onComplete,
        )
    }

    func walletImageModel(wallet: Wallet) -> WalletImageViewModel {
        WalletImageViewModel(wallet: wallet, source: .onboarding, avatarService: avatarService)
    }

    func dismiss() {
        onComplete?()
    }

    public func cancel() {
        guard outcome == .active else { return }
        outcome = .cancelled
        revision &+= 1
        creationTask?.cancel()
        creationTask = nil
        generatedWords = nil
        createdWallet = nil
    }

    func revealSecretPhrase() throws {
        try checkCreationIsActive()
        if generatedWords == nil {
            generatedWords = try generateSecretPhrase()
        }
    }

    func clearGeneratedWords() {
        generatedWords = nil
    }

    func finishCreation() {
        outcome = .completed
        revision &+= 1
        creationTask = nil
        generatedWords = nil
        createdWallet = nil
    }

    func checkCreationIsActive() throws {
        try Task.checkCancellation()
        guard outcome == .active else { throw CancellationError() }
    }

    func completeCreation(words: [String]) async throws -> WalletCreationCompletion {
        let task = try startCreation(words: words)
        return try await task.value
    }

    func startCreation(words: [String]) throws -> Task<WalletCreationCompletion, any Error> {
        try checkCreationIsActive()
        guard creationTask == nil else { throw CancellationError() }
        let currentRevision = revision
        let task = Task {
            defer {
                if currentRevision == revision {
                    creationTask = nil
                }
            }
            return try await performCreation(words: words, revision: currentRevision)
        }
        creationTask = task
        return task
    }

    private func performCreation(words: [String], revision currentRevision: UInt) async throws -> WalletCreationCompletion {
        try checkCreationIsActive()
        guard currentRevision == revision else { throw CancellationError() }
        let result: CreatedWallet
        if let createdWallet {
            result = createdWallet
        } else {
            result = try await createWallet(words: words)
            try checkCreationIsActive()
            guard currentRevision == revision else { throw CancellationError() }
            createdWallet = result
        }
        switch purpose {
        case .personal: return .personal(result)
        case let .dedicatedMultisigIdentity(onComplete):
            let wallet = try service.completeMultisigIdentityImport(result: result.importResult, source: .create)
            try checkCreationIsActive()
            guard currentRevision == revision else { throw CancellationError() }
            return .dedicatedMultisigIdentity(wallet.map(), onComplete: onComplete)
        }
    }
}

// MARK: - Actions

extension CreateWalletModel {
    func presentSelectImage(wallet: Wallet) {
        isPresentingSelectImageWallet = wallet
    }

    func generateSecretPhrase() throws -> [String] {
        try service.createWallet()
    }

    func createWallet(words: [String]) async throws -> CreatedWallet {
        let name = try await service.defaultWalletName(chain: .none)
        try checkCreationIsActive()
        let result = try await service.importWallet(
            name: name.name,
            import: service.importRequest(kind: .phrase, chain: nil, input: words.joined(separator: " "), nameRecord: nil),
            source: .create,
        )
        try checkCreationIsActive()
        preferences.acceptTerms()
        return CreatedWallet(importResult: result, hasExistingWallets: name.hasExistingWallets)
    }

    func activateWallet(wallet: Wallet) throws {
        try checkCreationIsActive()
        try service.setCurrentWalletId(walletId: wallet.id.id)
        finishCreation()
        dismiss()
    }

    func setupWalletComplete(wallet: Wallet) {
        do {
            try activateWallet(wallet: wallet)
        } catch {
            isPresentingAlertMessage = AlertMessage(title: Localized.Errors.errorOccurred, error: error)
        }
    }
}

enum WalletCreationCompletion: Sendable {
    case personal(CreatedWallet)
    case dedicatedMultisigIdentity(Wallet, onComplete: @MainActor @Sendable (Wallet) -> Void)
}

struct CreatedWallet: Sendable {
    let importResult: BsvHomeWalletImportResult
    let hasExistingWallets: Bool

    var wallet: Wallet {
        switch importResult {
        case let .new(wallet), let .existing(wallet): wallet.map()
        }
    }
}
