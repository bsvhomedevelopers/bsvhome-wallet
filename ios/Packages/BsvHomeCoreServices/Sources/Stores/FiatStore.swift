// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.FiatTransactionData
import protocol BsvHomeCore.BsvHomeFiatStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreFiatStore: BsvHomeFiatStore, @unchecked Sendable {
    private let store: FiatTransactionStore

    public init(store: FiatTransactionStore) {
        self.store = store
    }

    public func setTransactions(walletId: String, transactions: [BsvHomeCore.FiatTransactionData]) async throws {
        try store.setTransactions(
            walletId: WalletId.from(id: walletId),
            transactions: transactions.map { $0.map() },
        )
    }
}
