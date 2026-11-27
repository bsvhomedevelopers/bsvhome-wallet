// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeBsvAddressStoredState
import protocol BsvHomeCore.BsvHomeWalletStore
import struct BsvHomeCore.Wallet
import typealias BsvHomeCore.WalletId
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreWalletStore: BsvHomeWalletStore, @unchecked Sendable {
    private let store: WalletStore

    public init(store: WalletStore) {
        self.store = store
    }

    public func getWallets() throws -> [BsvHomeCore.Wallet] {
        try store.getWallets().map { $0.map() }
    }

    public func getWallet(walletId: BsvHomeCore.WalletId) throws -> BsvHomeCore.Wallet? {
        try store.getWallet(id: Primitives.WalletId.from(id: walletId)).map { $0.map() }
    }

    public func addWallet(wallet: BsvHomeCore.Wallet) async throws {
        try store.addWallet(wallet.map())
    }

    public func addWalletWithBsvAddresses(wallet: BsvHomeCore.Wallet, addresses: BsvHomeBsvAddressStoredState?) async throws {
        try store.addWallet(wallet.map(), bsvAddresses: addresses.map { (revision: $0.revision, value: $0.value) })
    }

    public func deleteWallet(walletId: BsvHomeCore.WalletId) async throws -> Bool {
        try store.deleteWallet(for: Primitives.WalletId.from(id: walletId))
    }

    public func setPinned(walletId: BsvHomeCore.WalletId, pinned: Bool) async throws {
        try store.pinWallet(Primitives.WalletId.from(id: walletId), value: pinned)
    }

    public func setImageUrl(walletId: BsvHomeCore.WalletId, imageUrl: String?) async throws {
        try store.setWalletAvatar(Primitives.WalletId.from(id: walletId), path: imageUrl)
    }

    public func setName(walletId: BsvHomeCore.WalletId, name: String) async throws {
        try store.renameWallet(Primitives.WalletId.from(id: walletId), name: name)
    }
}
