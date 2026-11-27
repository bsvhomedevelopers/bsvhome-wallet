// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeWalletDeletion
import enum BsvHomeCore.BsvHomeWalletImportType
import protocol BsvHomeCore.BsvHomeWalletServiceProtocol
import Primitives

public extension BsvHomeWalletServiceProtocol {
    var currentWalletId: WalletId? {
        do {
            return try currentWalletId().map { try WalletId.from(id: $0) }
        } catch {
            debugLog("current wallet id unavailable: \(error)")
            return .none
        }
    }

    func sorted(wallets: [Wallet]) -> [Wallet] {
        sortedWallets(wallets: wallets.map { $0.map() }).map { $0.map() }
    }

    func importWallet(name: String, type: BsvHomeWalletImportType, source: Primitives.WalletSource) async throws -> WalletImportResult {
        return switch try await importWallet(name: name, import: type, source: source.map()) {
        case let .new(wallet): .new(wallet.map())
        case let .existing(wallet): .existing(wallet.map())
        }
    }

    func delete(_ wallet: Wallet) async throws -> BsvHomeWalletDeletion {
        try await deleteWallet(walletId: wallet.id.id)
    }

    func setup(chains: [Chain]) async throws {
        _ = try await setupChains(chains: chains.map(\.rawValue))
    }

    func pin(wallet: Wallet) async throws {
        try await setPinned(walletId: wallet.id.id, pinned: true)
    }

    func unpin(wallet: Wallet) async throws {
        try await setPinned(walletId: wallet.id.id, pinned: false)
    }

    func rename(walletId: WalletId, newName: String) async throws {
        try await rename(walletId: walletId.id, name: newName)
    }

    func getWallets() async throws -> [Wallet] {
        try await wallets().map { $0.map() }
    }
}
