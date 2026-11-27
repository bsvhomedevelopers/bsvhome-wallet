// Copyright (c). Gem Wallet. All rights reserved.

import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit
import Testing
import Transactions

@MainActor
struct TransactionsFilterViewModelTests {
    @Test(arguments: [WalletType.privateKey, .view])
    func persistedTestTransactionsRemainOutsideWalletScopeAfterClearingFilters(walletType: WalletType) throws {
        let chains: [Chain] = [.bitcoinSv, .tron, .tronShasta]
        let db = DB.mockWithChains(chains)
        let wallet = Wallet.mock(
            id: .make(walletType: walletType, chain: .bitcoinSv, address: "sender"),
            type: walletType,
            accounts: chains.map { .mock(chain: $0) },
        )
        try WalletStore(db: db).addWallet(wallet)
        try TransactionStore(db: db).addTransactions(walletId: wallet.id, transactions: chains.map {
            .mock(id: TransactionId(chain: $0, hash: "stored"), assetId: $0.assetId)
        })
        let model = TransactionsFilterViewModel(wallet: wallet, chains: [.bitcoinSv, .tron], type: .all)
        model.query.request.base.filters = []
        let stored = try db.dbQueue.read { try TransactionsRequest(walletId: wallet.id, type: .all, allowedChains: chains, environment: .localDevelopment).fetch($0) }
        let transactions = try db.dbQueue.read { try model.query.request.base.fetch($0) }

        #expect(Set(stored.map(\.transaction.assetId)) == Set(chains.map(\.assetId)))
        #expect(transactions.map(\.transaction.assetId) == [Chain.bitcoinSv.assetId])

        model.query.request.base.filters = [.chains([Chain.tronShasta.rawValue])]
        let rejected = try db.dbQueue.read { try model.query.request.base.fetch($0) }
        #expect(rejected.isEmpty)
    }
}
