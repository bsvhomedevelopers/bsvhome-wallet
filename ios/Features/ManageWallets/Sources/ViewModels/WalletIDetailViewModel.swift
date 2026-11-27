import Components
import enum BsvHomeCore.BsvHomeBsvColdWalletRole
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import enum BsvHomeCore.BsvHomeWalletSecret
import enum BsvHomeCore.BsvHomeWalletSecretKind
import protocol BsvHomeCore.BsvHomeWalletServiceProtocol
import func BsvHomeCore.walletRow
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import Onboarding
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI

@Observable
@MainActor
public final class WalletDetailViewModel {
    private let navigationPath: Binding<NavigationPath>
    private let service: any BsvHomeWalletServiceProtocol
    private let preferences: ObservablePreferences
    private let deletionWarning: @MainActor (Wallet) -> String?
    private let authenticateSecret: (@MainActor () async throws -> Void)?
    private var secretSession = UUID()
    private let coldRoleProvider: (@MainActor (Wallet) throws -> BsvHomeBsvColdWalletRole)?
    private let pairingAvailability: (@MainActor (Wallet) async throws -> Bool)?
    private let onOpenPairing: (@MainActor (Wallet) -> Void)?
    private let onOpenSigning: (@MainActor (Wallet) -> Void)?
    private var coldPresentationRevision = UUID()
    private var classifiedColdWallet: Wallet?
    private var loadedColdRole: BsvHomeBsvColdWalletRole?
    private var pairingAvailable = false

    var isLoadingColdPresentation = false
    var coldPresentationError: String?
    var hasColdPresentation: Bool { coldRoleProvider != nil || pairingAvailability != nil || onOpenPairing != nil || onOpenSigning != nil }
    private var currentColdCapabilityAvailable: Bool {
        classifiedColdWallet == walletQuery.value && !isLoadingColdPresentation && coldPresentationError == nil && pairingAvailable
    }

    var canOpenColdPairing: Bool { onOpenPairing != nil && currentColdCapabilityAvailable }
    var canOpenColdSigning: Bool { onOpenSigning != nil && currentColdCapabilityAvailable }

    var coldRoleTitle: String? {
        guard classifiedColdWallet == walletQuery.value, coldPresentationError == nil else { return nil }
        if loadedColdRole == .signer {
            return Localized.coldWalletTitle
        }
        if loadedColdRole == .observer {
            return Localized.Info.WatchWallet.title
        }
        return nil
    }

    var nameInput: String
    var isPresentingAlertMessage: AlertMessage?
    var isPresentingDeleteConfirmation: Bool?
    var isPresentingExportWallet: BsvHomeWalletSecret?
    var secretKind: BsvHomeWalletSecretKind?
    var secretMetadataError: String?
    var isRevealingSecret = false

    public let walletQuery: ObservableQuery<WalletRequest>
    public var wallet: Wallet {
        BsvHomeRuntimeEnvironment.current.visibleWallet(wallet: walletQuery.value.map()).map()
    }

    public init(
        navigationPath: Binding<NavigationPath>,
        wallet: Wallet,
        service: any BsvHomeWalletServiceProtocol,
        preferences: ObservablePreferences,
        deletionWarning: @escaping @MainActor (Wallet) -> String? = { _ in nil },
        authenticateSecret: (@MainActor () async throws -> Void)? = nil,
        coldRole: (@MainActor (Wallet) throws -> BsvHomeBsvColdWalletRole)? = nil,
        canOpenPairing: (@MainActor (Wallet) async throws -> Bool)? = nil,
        onOpenPairing: (@MainActor (Wallet) -> Void)? = nil,
        onOpenSigning: (@MainActor (Wallet) -> Void)? = nil,
    ) {
        self.navigationPath = navigationPath
        self.service = service
        self.preferences = preferences
        self.deletionWarning = deletionWarning
        self.authenticateSecret = authenticateSecret
        coldRoleProvider = coldRole
        pairingAvailability = canOpenPairing
        self.onOpenPairing = onOpenPairing
        self.onOpenSigning = onOpenSigning
        nameInput = wallet.name
        isPresentingAlertMessage = nil
        isPresentingDeleteConfirmation = nil
        isPresentingExportWallet = nil
        walletQuery = ObservableQuery(WalletRequest(walletId: wallet.id), initialValue: wallet)
    }

    var name: String {
        wallet.name
    }

    var walletDeleteWarning: String? { deletionWarning(wallet) }

    var title: String {
        Localized.Common.wallet
    }

    var address: WalletDetailAddress? {
        guard BsvHomeRuntimeEnvironment.current.allowsWallet(wallet: walletQuery.value.map()) else { return nil }
        return switch walletRow(wallet: wallet.map()).subtitle {
        case .multicoin: .none
        case let .account(chain, address): .account(SimpleAccount(name: .none, chain: Primitives.Chain(core: chain), address: address, assetImage: .none))
        }
    }

    func addressLink(account: SimpleAccount) -> BlockExplorerLink {
        service.addressUrl(chain: account.chain.rawValue, address: account.address).map()
    }

    func avatarAssetImage(for wallet: Wallet) -> AssetImage {
        let avatar = WalletViewModel(wallet: wallet).avatarImage
        return AssetImage(
            type: avatar.type,
            imageURL: avatar.imageURL,
            placeholder: avatar.placeholder,
            chainPlaceholder: Images.Wallets.editFilled,
        )
    }
}

// MARK: - Business Logic

extension WalletDetailViewModel {
    func loadColdPresentation() async {
        guard !Task.isCancelled else { return }
        let revision = UUID()
        coldPresentationRevision = revision
        let expected = walletQuery.value
        classifiedColdWallet = nil
        loadedColdRole = nil
        pairingAvailable = false
        coldPresentationError = nil
        isLoadingColdPresentation = hasColdPresentation
        defer {
            if coldPresentationRevision == revision {
                isLoadingColdPresentation = false
            }
        }
        guard hasColdPresentation else { return }
        do {
            let role = try coldRoleProvider?(expected)
            let available = try await pairingAvailability?(expected) ?? false
            guard coldPresentationRevision == revision, walletQuery.value == expected, !Task.isCancelled else { return }
            loadedColdRole = role
            pairingAvailable = available
            classifiedColdWallet = expected
        } catch is CancellationError {
            return
        } catch {
            guard coldPresentationRevision == revision, walletQuery.value == expected, !Task.isCancelled else { return }
            coldPresentationError = Localized.Errors.errorOccurred
        }
    }

    func onSelectColdPairing() {
        guard canOpenColdPairing else { return }
        onOpenPairing?(walletQuery.value)
    }

    func onSelectColdSigning() {
        guard canOpenColdSigning else { return }
        onOpenSigning?(walletQuery.value)
    }

    func rename(name: String) async throws {
        try await service.rename(walletId: wallet.id, newName: name)
    }

    func delete() async throws {
        switch try await service.delete(wallet) {
        case .walletsRemaining: break
        case .lastWalletDeleted: preferences.reload()
        }
    }

    func onSelectImage() {
        navigationPath.wrappedValue.append(Scenes.WalletSelectImage(wallet: wallet))
    }
}

// MARK: - Actions

extension WalletDetailViewModel {
    func onChangeWalletName() async {
        do {
            try await rename(name: nameInput)
        } catch {
            isPresentingAlertMessage = AlertMessage(message: error.localizedDescription)
        }
    }

    func onShowSecret() {
        guard !isRevealingSecret, secretKind != nil, secretMetadataError == nil,
              UIApplication.shared.applicationState == .active,
              UIApplication.shared.isProtectedDataAvailable, !UIScreen.main.isCaptured
        else { return }
        isRevealingSecret = true
        let expectedWallet = wallet.id
        let session = secretSession
        Task {
            defer { isRevealingSecret = false }
            do {
                guard let authenticateSecret else { throw AnyError(Localized.bsvHdBackupUnavailable) }
                try await authenticateSecret()
                guard canRevealSecret(walletId: expectedWallet, session: session) else { return }
                let secret = try await service.exportSecret(walletId: expectedWallet.id)
                guard canRevealSecret(walletId: expectedWallet, session: session) else { return }
                isPresentingExportWallet = secret
            } catch is CancellationError {
                return
            } catch {
                if session == secretSession {
                    isPresentingAlertMessage = AlertMessage(error: error)
                }
            }
        }
    }

    func loadSecretKind() async {
        let expected = wallet.id
        do {
            let kind = try await service.secretKind(walletId: expected.id)
            guard wallet.id == expected else { return }
            secretKind = kind
            secretMetadataError = nil
        } catch {
            guard wallet.id == expected else { return }
            secretKind = nil
            secretMetadataError = Localized.bsvHdBackupUnavailable
        }
    }

    func clearSecrets() {
        secretSession = UUID()
        isPresentingExportWallet = nil
    }

    private func canRevealSecret(walletId: WalletId, session: UUID) -> Bool {
        session == secretSession && wallet.id == walletId && UIApplication.shared.applicationState == .active
            && UIApplication.shared.isProtectedDataAvailable && !UIScreen.main.isCaptured
    }

    func onSelectDelete() {
        isPresentingDeleteConfirmation = true
    }

    func onDelete() async -> Bool {
        do {
            try await delete()
            return true
        } catch {
            isPresentingAlertMessage = AlertMessage(message: error.localizedDescription)
            return false
        }
    }
}
