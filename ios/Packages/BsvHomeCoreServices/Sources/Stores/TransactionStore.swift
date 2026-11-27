// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import typealias BsvHomeCore.AssetId
import func BsvHomeCore.canReplaceStoredTransaction
import protocol BsvHomeCore.BsvHomeTransactionStore
import typealias BsvHomeCore.Transaction
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreTransactionStore: BsvHomeTransactionStore, @unchecked Sendable {
    private let store: TransactionStore

    public init(store: TransactionStore) {
        self.store = store
    }

    public func saveTransactions(walletId: String, transactions: [BsvHomeCore.Transaction]) async throws {
        try store.addTransactions(
            walletId: WalletId.from(id: walletId),
            transactions: transactions.map { $0.map() },
            shouldReplace: { chain, previous, next in
                canReplaceStoredTransaction(chain: chain.map(), previousState: previous.map(), nextState: next.map())
            },
        )
    }
}
