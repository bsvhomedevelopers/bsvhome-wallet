// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeAppStartServiceProtocol
import protocol BsvHomeCore.BsvHomeAvatarServiceProtocol
import protocol BsvHomeCore.BsvHomeKeystoreProtocol
import protocol BsvHomeCore.BsvHomeNameServiceProtocol
import enum BsvHomeCore.BsvHomeWalletImportKind
import protocol BsvHomeCore.BsvHomeWalletServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Primitives
import PrimitivesComponents
import SwiftUI

@Observable
@MainActor
public final class ImportWalletViewModel {
    private let service: any BsvHomeWalletServiceProtocol
    private let keystore: any BsvHomeKeystoreProtocol
    private let authenticate: @MainActor @Sendable () async throws -> Void
    let onColdWallet: VoidAction
    private let appStartService: any BsvHomeAppStartServiceProtocol
    private let preferences: ObservablePreferences
    private let nameService: any BsvHomeNameServiceProtocol
    private let avatarService: any BsvHomeAvatarServiceProtocol
    let onComplete: VoidAction
    let purpose: WalletOnboardingPurpose
    private weak var activeInputModel: ImportWalletSceneViewModel?

    var isPresentingSelectImageWallet: Wallet?

    public init(
        service: any BsvHomeWalletServiceProtocol,
        keystore: any BsvHomeKeystoreProtocol,
        authenticate: @escaping @MainActor @Sendable () async throws -> Void,
        onColdWallet: VoidAction = nil,
        appStartService: any BsvHomeAppStartServiceProtocol,
        preferences: ObservablePreferences,
        nameService: any BsvHomeNameServiceProtocol,
        avatarService: any BsvHomeAvatarServiceProtocol,
        onComplete: VoidAction,
        purpose: WalletOnboardingPurpose = .personal,
    ) {
        self.service = service
        self.keystore = keystore
        self.authenticate = authenticate
        self.onColdWallet = onColdWallet
        self.appStartService = appStartService
        self.preferences = preferences
        self.nameService = nameService
        self.avatarService = avatarService
        self.onComplete = onComplete
        self.purpose = purpose
    }

    public var isAcceptTermsCompleted: Bool {
        preferences.isAcceptTermsCompleted
    }

    func importWalletModel(type: ImportWalletType, onComplete: @escaping @MainActor @Sendable (ImportWalletSceneResult) -> Void) -> ImportWalletSceneViewModel {
        ImportWalletSceneViewModel(service: service, keystore: keystore, authenticate: authenticate, appStartService: appStartService, preferences: preferences, nameService: nameService, type: type, onComplete: onComplete, purpose: purpose)
    }

    func registerInputModel(_ model: ImportWalletSceneViewModel) {
        if activeInputModel !== model {
            activeInputModel?.clearSensitiveState()
        }
        activeInputModel = model
    }

    public func cancel() {
        activeInputModel?.clearSensitiveState()
    }

    func importWalletTypeModel() -> ImportWalletTypeViewModel {
        ImportWalletTypeViewModel(service: service)
    }

    func supportedChains(for kind: BsvHomeWalletImportKind) -> [Chain] {
        service.importChains(kind: kind, query: "").map { Chain(core: $0) }
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
}

// MARK: - Actions

extension ImportWalletViewModel {
    func presentSelectImage(wallet: Wallet) {
        isPresentingSelectImageWallet = wallet
    }
}
