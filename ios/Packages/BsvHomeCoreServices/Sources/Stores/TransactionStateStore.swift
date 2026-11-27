// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import func BsvHomeCore.canReplaceStoredTransaction
import struct BsvHomeCore.BsvHomePendingTransaction
import protocol BsvHomeCore.BsvHomeTransactionStateStore
import struct BsvHomeCore.BsvHomeTransactionStateUpdate
import typealias BsvHomeCore.Transaction
import typealias BsvHomeCore.TransactionId
import typealias BsvHomeCore.TransactionState
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreTransactionStateStore: BsvHomeTransactionStateStore, @unchecked Sendable {
    private let store: TransactionStore
    private let walletStore: WalletStore

    public init(store: TransactionStore, walletStore: WalletStore) {
        self.store = store
        self.walletStore = walletStore
    }

    public func getPendingTransactions() async throws -> [BsvHomePendingTransaction] {
        let wallets = try Dictionary(uniqueKeysWithValues: walletStore.getWallets().map { ($0.id, $0) })
        return try store.getTransactions(states: [.pending, .inTransit]).flatMap { walletId, transactions -> [BsvHomePendingTransaction] in
            guard let wallet = wallets[walletId] else { return [] }
            return transactions.map { BsvHomePendingTransaction(wallet: wallet.map(), transaction: $0.map()) }
        }
    }

    public func getTransaction(walletId: String, transactionId: BsvHomeCore.TransactionId) async throws -> BsvHomePendingTransaction? {
        let walletId = try WalletId.from(id: walletId)
        let transactionId = try Primitives.TransactionId(id: transactionId)
        guard
            let wallet = try walletStore.getWallet(id: walletId),
            try store.getTransactionState(walletId: walletId, transactionId: transactionId) != nil
        else { return nil }
        let transaction = try store.getTransaction(walletId: walletId, transactionId: transactionId).transaction
        return BsvHomePendingTransaction(wallet: wallet.map(), transaction: transaction.map())
    }

    public func addTransactions(walletId: String, transactions: [BsvHomeCore.Transaction]) async throws {
        try store.addTransactions(
            walletId: WalletId.from(id: walletId),
            transactions: transactions.map { $0.map() },
            shouldReplace: { chain, previous, next in
                canReplaceStoredTransaction(chain: chain.map(), previousState: previous.map(), nextState: next.map())
            },
        )
    }

    public func getState(walletId: String, transactionId: BsvHomeCore.TransactionId) async throws -> BsvHomeCore.TransactionState? {
        try store.getTransactionState(walletId: WalletId.from(id: walletId), transactionId: Primitives.TransactionId(id: transactionId)).map { $0.map() }
    }

    public func updateTransactionHash(walletId: String, transactionId: BsvHomeCore.TransactionId, hash: String) async throws {
        try store.updateTransactionHash(
            walletId: WalletId.from(id: walletId),
            transactionId: Primitives.TransactionId(id: transactionId),
            hash: hash,
        )
    }

    public func deleteTransaction(walletId: String, transactionId: BsvHomeCore.TransactionId) async throws {
        try store.deleteTransaction(walletId: WalletId.from(id: walletId), transactionId: Primitives.TransactionId(id: transactionId))
    }

    public func updateTransaction(walletId: String, transactionId: BsvHomeCore.TransactionId, update: BsvHomeTransactionStateUpdate) async throws -> Bool {
        try store.updateTransaction(
            walletId: WalletId.from(id: walletId),
            transactionId: Primitives.TransactionId(id: transactionId),
            state: update.state.map(),
            fee: update.fee?.description,
            blockNumber: update.blockNumber.flatMap { Int($0) },
            metadata: update.metadata,
            confirmationEtaSeconds: update.confirmationEtaSeconds,
            shouldReplace: { chain, previous, next in
                canReplaceStoredTransaction(chain: chain.map(), previousState: previous.map(), nextState: next.map())
            },
        ) > 0
    }
}
