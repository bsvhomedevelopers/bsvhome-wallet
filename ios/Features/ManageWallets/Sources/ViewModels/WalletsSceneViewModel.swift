import Components
import Foundation
import enum BsvHomeCore.BsvHomeBsvColdWalletRole
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import protocol BsvHomeCore.BsvHomeWalletServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import Primitives
import PrimitivesComponents
import Store
import SwiftUI

@Observable
@MainActor
public final class WalletsSceneViewModel {
    private let service: any BsvHomeWalletServiceProtocol
    private let preferences: ObservablePreferences
    private let isPresentingCreateWalletSheet: Binding<Bool>
    private let isPresentingImportWalletSheet: Binding<Bool>
    private let navigationPath: Binding<NavigationPath>
    private let deletionWarning: @MainActor (Wallet) -> String?
    private let coldRoleProvider: (@MainActor (Wallet) throws -> BsvHomeBsvColdWalletRole)?
    private var classifiedWallets: [Wallet]?
    private var coldRoles: [WalletId: BsvHomeBsvColdWalletRole] = [:]

    public private(set) var coldRoleError: String?
    public var coldRoleInput: [Wallet] { pinnedWalletsQuery.value + walletsQuery.value }
    public var coldRolesReady: Bool { coldRoleProvider == nil || (coldRoleError == nil && classifiedWallets == coldRoleInput) }
    public var personalPinnedWallets: [Wallet] { coldRolesReady ? pinnedWallets.filter { coldRole(for: $0) != .signer } : [] }
    public var personalWallets: [Wallet] { coldRolesReady ? wallets.filter { coldRole(for: $0) != .signer } : [] }
    public var coldSignerWallets: [Wallet] { coldRolesReady ? (pinnedWallets + wallets).filter { coldRole(for: $0) == .signer } : [] }

    public var isPresentingAlertMessage: AlertMessage?
    var walletDelete: Wallet?

    public var currentWalletId: WalletId? {
        service.currentWalletId
    }

    public let pinnedWalletsQuery: ObservableQuery<WalletsRequest>
    public let walletsQuery: ObservableQuery<WalletsRequest>

    public var pinnedWallets: [Wallet] { sorted(pinnedWalletsQuery.value) }
    public var wallets: [Wallet] { sorted(walletsQuery.value) }
    var hasWallets: Bool { wallets.isNotEmpty || pinnedWallets.isNotEmpty }

    public init(
        navigationPath: Binding<NavigationPath>,
        walletService: any BsvHomeWalletServiceProtocol,
        preferences: ObservablePreferences,
        isPresentingCreateWalletSheet: Binding<Bool>,
        isPresentingImportWalletSheet: Binding<Bool>,
        deletionWarning: @escaping @MainActor (Wallet) -> String? = { _ in nil },
        coldRole: (@MainActor (Wallet) throws -> BsvHomeBsvColdWalletRole)? = nil,
    ) {
        self.navigationPath = navigationPath
        self.deletionWarning = deletionWarning
        coldRoleProvider = coldRole
        service = walletService
        self.preferences = preferences
        isPresentingAlertMessage = nil
        walletDelete = nil
        self.isPresentingCreateWalletSheet = isPresentingCreateWalletSheet
        self.isPresentingImportWalletSheet = isPresentingImportWalletSheet
        pinnedWalletsQuery = ObservableQuery(WalletsRequest(isPinned: true), initialValue: [])
        walletsQuery = ObservableQuery(WalletsRequest(isPinned: false), initialValue: [])
    }

    public func coldRole(for wallet: Wallet) -> BsvHomeBsvColdWalletRole? {
        guard coldRolesReady else { return nil }
        guard coldRoleProvider != nil else { return BsvHomeBsvColdWalletRole.none }
        return coldRoles[wallet.id]
    }

    public func refreshColdRoles() {
        let wallets = coldRoleInput
        do {
            var roles: [WalletId: BsvHomeBsvColdWalletRole] = [:]
            if let coldRoleProvider {
                for wallet in wallets {
                    roles[wallet.id] = try coldRoleProvider(wallet)
                }
            }
            coldRoles = roles
            classifiedWallets = wallets
            coldRoleError = nil
        } catch {
            coldRoles = [:]
            classifiedWallets = nil
            coldRoleError = Localized.Errors.errorOccurred
        }
    }

    var title: String {
        Localized.Wallets.title
    }

    var walletDeleteWarning: String? { walletDelete.flatMap(deletionWarning) }

    private func sorted(_ wallets: [Wallet]) -> [Wallet] {
        service.sorted(wallets: wallets
            .filter { BsvHomeRuntimeEnvironment.current.allowsWallet(wallet: $0.map()) }
            .map { BsvHomeRuntimeEnvironment.current.visibleWallet(wallet: $0.map()).map() })
    }
}

// MARK: - Business Logic

extension WalletsSceneViewModel {
    @discardableResult
    func setCurrent(_ walletId: WalletId) -> Bool {
        do {
            try service.setCurrentWalletId(walletId: walletId.id)
            isPresentingAlertMessage = nil
            return true
        } catch {
            isPresentingAlertMessage = AlertMessage(error: error)
            return false
        }
    }

    func onEdit(wallet: Wallet) {
        navigationPath.wrappedValue.append(Scenes.WalletDetail(wallet: wallet))
    }

    private func delete(_ wallet: Wallet) async throws {
        switch try await service.delete(wallet) {
        case .walletsRemaining: break
        case .lastWalletDeleted: preferences.reload()
        }
    }

    private func pin(_ wallet: Wallet) async throws {
        if wallet.isPinned {
            try await service.unpin(wallet: wallet)
        } else {
            try await service.pin(wallet: wallet)
        }
    }
}

// MARK: - Actions

extension WalletsSceneViewModel {
    func onSelectCreateWallet() {
        isPresentingCreateWalletSheet.wrappedValue.toggle()
    }

    func onSelectImportWallet() {
        isPresentingImportWalletSheet.wrappedValue.toggle()
    }

    func onSelect(wallet: Wallet, dismiss: DismissAction) {
        onSelect(wallet: wallet) { dismiss() }
    }

    public func onSelect(wallet: Wallet, onSelected: () -> Void) {
        refreshColdRoles()
        guard coldRolesReady else {
            isPresentingAlertMessage = AlertMessage(message: coldRoleError ?? Localized.Errors.errorOccurred)
            return
        }
        guard setCurrent(wallet.id) else { return }
        onSelected()
    }

    func onChangeWallets(dismiss: DismissAction) {
        guard !hasWallets else { return }
        dismiss()
    }

    func onDelete(wallet: Wallet) {
        walletDelete = wallet
    }

    func onPin(wallet: Wallet) async {
        do {
            try await pin(wallet)
        } catch {
            isPresentingAlertMessage = AlertMessage(error: error)
        }
    }

    func onDeleteConfirmed(wallet: Wallet) async {
        do {
            try await delete(wallet)
        } catch {
            isPresentingAlertMessage = AlertMessage(error: error)
        }
    }
}
