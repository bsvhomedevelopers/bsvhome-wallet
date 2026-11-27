// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import GRDB
import Primitives
import PrimitivesTestKit
@testable import Store
import StoreTestKit
import Testing

struct TransactionStoreTests {
    @Test func transactionsByStateAreKeyedByWallet() throws {
        let asset = AssetBasic.mock(asset: .mock(id: Chain.robinhood.assetId))
        let db = DB.mockAssets(assets: [asset])
        let walletStore = WalletStore(db: db)
        let walletId = WalletId.single(chain: .robinhood, address: "0xsender")
        try walletStore.addWallet(.mock(id: walletId, type: .single, accounts: [.mock(chain: .robinhood, address: "0xsender")]))

        let store = TransactionStore(db: db)
        let transactionId = TransactionId(chain: .robinhood, hash: "hash")
        try store.addTransactions(walletId: walletId, transactions: [
            .mock(id: transactionId, state: .pending, assetId: Chain.robinhood.assetId),
        ])

        let transactions = try store.getTransactions(states: [.pending])

        #expect(transactions.keys.contains(walletId))
        #expect(transactions[walletId]?.map(\.id) == [transactionId])
        #expect(try store.getTransactions(states: [.confirmed]).isEmpty)
    }

    @Test func assetAssociationsReplaced() throws {
        let btc = AssetId(chain: .bitcoin, tokenId: nil)
        let eth = AssetId(chain: .ethereum, tokenId: nil)
        let sol = AssetId(chain: .solana, tokenId: nil)

        let assets: [AssetBasic] = [
            .mock(asset: .mock(id: btc)),
            .mock(asset: .mock(id: eth)),
            .mock(asset: .mock(id: sol)),
        ]

        let db = DB.mockAssets(assets: assets)
        let walletStore = WalletStore(db: db)
        let walletId = WalletId.multicoin(address: "test")
        try walletStore.addWallet(.mock(id: walletId, accounts: assets.map { Account.mock(chain: $0.asset.chain) }))

        let store = TransactionStore(db: db)
        let transactionId = TransactionId(chain: .ethereum, hash: "1")
        try store.addTransactions(walletId: walletId, transactions: [
            .mock(
                id: transactionId,
                type: .swap,
                assetId: btc,
                metadata: .encode(TransactionSwapMetadata(
                    fromAsset: btc, fromValue: "100", toAsset: eth, toValue: "200", provider: nil,
                )),
            ),
        ])

        try store.addTransactions(walletId: walletId, transactions: [
            .mock(
                id: transactionId,
                type: .swap,
                assetId: btc,
                metadata: .encode(TransactionSwapMetadata(
                    fromAsset: btc, fromValue: "100", toAsset: sol, toValue: "300", provider: nil,
                )),
            ),
        ])

        let assetIds = try store.getTransactionAssetAssociations(for: transactionId).map(\.assetId)

        #expect(assetIds.count == 2)
        #expect(Set(assetIds) == Set([btc, sol]))
    }

    @Test func mergePreservesIdentityAndAssociationsWithinWallet() throws {
        let ethereum = Chain.ethereum.assetId
        let bitcoin = Chain.bitcoin.assetId
        let solana = Chain.solana.assetId
        let db = DB.mockAssets(assets: [ethereum, bitcoin, solana].map { .mock(asset: .mock(id: $0)) })
        let store = TransactionStore(db: db)
        let walletId = WalletId.mock()
        let otherWalletId = WalletId.multicoin(address: "other")
        try WalletStore(db: db).addWallet(.mock(id: otherWalletId))
        let sourceId = TransactionId(chain: .ethereum, hash: "pending")
        let targetId = TransactionId(chain: .ethereum, hash: "confirmed")
        let source = Transaction.mock(
            id: sourceId,
            type: .swap,
            state: .pending,
            assetId: ethereum,
            metadata: .encode(TransactionSwapMetadata(fromAsset: ethereum, fromValue: "100", toAsset: bitcoin, toValue: "200", provider: nil)),
        )
        let target = Transaction.mock(
            id: targetId,
            type: .swap,
            assetId: ethereum,
            fee: "42",
            metadata: .encode(TransactionSwapMetadata(fromAsset: ethereum, fromValue: "100", toAsset: solana, toValue: "300", provider: nil)),
        )
        try store.addTransactions(walletId: walletId, transactions: [source, target])
        try store.addTransactions(walletId: otherWalletId, transactions: [source, target])
        let storedSource = try store.getTransaction(walletId: walletId, transactionId: sourceId)
        let storedTarget = try store.getTransaction(walletId: walletId, transactionId: targetId)
        let sourceRecord = try #require(try db.dbQueue.read {
            try TransactionRecord.filter(TransactionRecord.Columns.walletId == walletId.id)
                .filter(TransactionRecord.Columns.transactionId == sourceId.identifier).fetchOne($0)
        })

        try store.updateTransactionHash(walletId: walletId, transactionId: sourceId, hash: targetId.hash)
        try store.updateTransactionHash(walletId: walletId, transactionId: sourceId, hash: targetId.hash)
        try store.updateTransactionHash(walletId: walletId, transactionId: targetId, hash: targetId.hash)

        let result = try store.getTransaction(walletId: walletId, transactionId: targetId)
        #expect(result.recordId == storedSource.recordId)
        #expect(throws: RecordError.self) {
            try db.dbQueue.read { try TransactionRequest(walletId: otherWalletId, recordId: result.recordId, environment: .production).fetch($0) }
        }
        #expect(result.transaction == storedTarget.transaction)
        #expect(Set(result.assets.map(\.id)) == Set([ethereum, solana]))
        #expect(try store.getTransaction(walletId: otherWalletId, transactionId: sourceId).transaction == storedSource.transaction)
        #expect(try store.getTransaction(walletId: otherWalletId, transactionId: targetId).transaction == storedTarget.transaction)
        try db.dbQueue.read { db in
            let record = try TransactionRecord.filter(TransactionRecord.Columns.walletId == walletId.id).fetchOne(db)
            let count = try TransactionRecord.fetchCount(db)
            #expect(record?.id == sourceRecord.id)
            #expect(count == 3)
        }
    }

    @Test func rejectedReplacementRetainsRecordAndAssociations() throws {
        let bsv = Chain.bitcoinSv.assetId
        let ethereum = Chain.ethereum.assetId
        let solana = Chain.solana.assetId
        let db = DB.mockAssets(assets: [bsv, ethereum, solana].map { .mock(asset: .mock(id: $0)) })
        let store = TransactionStore(db: db)
        let walletId = WalletId.mock()
        let transactionId = TransactionId(chain: .bitcoinSv, hash: "confirmed")
        let original = Transaction.mock(
            id: transactionId, type: .swap, state: .confirmed, assetId: bsv,
            from: "original-sender", to: "original-recipient", value: "100", fee: "17", memo: "original-memo",
            metadata: .encode(TransactionSwapMetadata(fromAsset: bsv, fromValue: "100", toAsset: ethereum, toValue: "200", provider: nil)),
        )
        try store.addTransactions(walletId: walletId, transactions: [original])
        let before = try recordData(db: db, walletId: walletId, transactionId: transactionId)
        let associations = try store.getTransactionAssetAssociations(for: transactionId).map(\.assetId)
        let stale = Transaction.mock(
            id: transactionId, type: .swap, state: .pending, assetId: bsv, value: "999", fee: "0",
            metadata: .encode(TransactionSwapMetadata(fromAsset: bsv, fromValue: "999", toAsset: solana, toValue: "1", provider: nil)),
        )

        try store.addTransactions(walletId: walletId, transactions: [stale], shouldReplace: { chain, previous, next in
            #expect(chain == .bitcoinSv)
            #expect(previous == .confirmed)
            #expect(next == .pending)
            return false
        })

        #expect(try recordData(db: db, walletId: walletId, transactionId: transactionId) == before)
        #expect(try store.getTransactionAssetAssociations(for: transactionId).map(\.assetId) == associations)
    }

    @Test func replacementIsScopedAndSameStateUpdatesRemainAllowed() throws {
        let bsv = Chain.bitcoinSv.assetId
        let db = DB.mockAssets(assets: [.mock(asset: .mock(id: bsv))])
        let store = TransactionStore(db: db)
        let walletId = WalletId.mock()
        let otherWallet = WalletId.multicoin(address: "other-wallet")
        try WalletStore(db: db).addWallet(.mock(id: otherWallet))
        let transactionId = TransactionId(chain: .bitcoinSv, hash: "shared-hash")
        let otherId = TransactionId(chain: .bitcoinSv, hash: "other-hash")
        try store.addTransactions(walletId: walletId, transactions: [.mock(id: transactionId, state: .confirmed, assetId: bsv, fee: "17")])
        let pending = Transaction.mock(id: transactionId, state: .pending, assetId: bsv, value: "20")

        try store.addTransactions(walletId: otherWallet, transactions: [pending], shouldReplace: { _, _, _ in false })
        try store.addTransactions(walletId: walletId, transactions: [.mock(id: otherId, state: .pending, assetId: bsv)], shouldReplace: { _, _, _ in false })
        #expect(try store.getTransactionState(walletId: walletId, transactionId: transactionId) == .confirmed)
        #expect(try store.getTransactionState(walletId: otherWallet, transactionId: transactionId) == .pending)
        #expect(try store.getTransactionState(walletId: walletId, transactionId: otherId) == .pending)

        try store.addTransactions(
            walletId: walletId,
            transactions: [.mock(id: transactionId, state: .confirmed, assetId: bsv, value: "30", fee: "42", memo: "refreshed")],
            shouldReplace: { _, previous, next in
                #expect(previous == .confirmed)
                #expect(next == .confirmed)
                return true
            },
        )
        let refreshed = try store.getTransaction(walletId: walletId, transactionId: transactionId).transaction
        #expect(refreshed.state == .confirmed)
        #expect(refreshed.value == "30")
        #expect(refreshed.fee == "42")
        #expect(refreshed.memo == "refreshed")
        #expect(try store.getTransaction(walletId: otherWallet, transactionId: transactionId).transaction.value == "20")

        try store.addTransactions(walletId: walletId, transactions: [pending])
        #expect(try store.getTransactionState(walletId: walletId, transactionId: transactionId) == .pending)
    }

    @Test func orderedWritesUseLatestStateAndRejectedUpdatesKeepMetadata() throws {
        let bsv = Chain.bitcoinSv.assetId
        let db = DB.mockAssets(assets: [.mock(asset: .mock(id: bsv))])
        let store = TransactionStore(db: db)
        let walletId = WalletId.mock()
        let transactionId = TransactionId(chain: .bitcoinSv, hash: "ordered-hash")
        let pending = Transaction.mock(id: transactionId, state: .pending, assetId: bsv, fee: "1", memo: "stale")
        let confirmed = Transaction.mock(id: transactionId, state: .confirmed, assetId: bsv, fee: "42", memo: "fresh")
        try store.addTransactions(walletId: walletId, transactions: [pending])

        try store.addTransactions(walletId: walletId, transactions: [confirmed, pending], shouldReplace: { _, previous, _ in previous != .confirmed })

        let transaction = try store.getTransaction(walletId: walletId, transactionId: transactionId).transaction
        #expect(transaction.state == .confirmed)
        #expect(transaction.fee == "42")
        #expect(transaction.memo == "fresh")
        let before = try recordData(db: db, walletId: walletId, transactionId: transactionId)
        let updated = try store.updateTransaction(
            walletId: walletId, transactionId: transactionId, state: .pending,
            fee: "0", blockNumber: 0, metadata: "{}", confirmationEtaSeconds: 10,
            shouldReplace: { _, previous, next in
                #expect(previous == .confirmed)
                #expect(next == .pending)
                return false
            },
        )
        #expect(updated == 0)
        #expect(try recordData(db: db, walletId: walletId, transactionId: transactionId) == before)
    }

    @Test func invalidStoredStateThrowsAndRollsBackBatch() throws {
        let bsv = Chain.bitcoinSv.assetId
        let db = DB.mockAssets(assets: [.mock(asset: .mock(id: bsv))])
        let store = TransactionStore(db: db)
        let walletId = WalletId.mock()
        let firstId = TransactionId(chain: .bitcoinSv, hash: "first")
        let invalidId = TransactionId(chain: .bitcoinSv, hash: "invalid")
        try store.addTransactions(walletId: walletId, transactions: [firstId, invalidId].map { .mock(id: $0, state: .pending, assetId: bsv) })
        try db.dbQueue.write { db in
            _ = try TransactionRecord.filter(TransactionRecord.Columns.walletId == walletId.id)
                .filter(TransactionRecord.Columns.transactionId == invalidId.identifier)
                .updateAll(db, [TransactionRecord.Columns.state.set(to: "invalid-state")])
        }
        let before = try recordData(db: db, walletId: walletId, transactionId: firstId)

        #expect(throws: DecodingError.self) { try store.getTransactionState(walletId: walletId, transactionId: invalidId) }
        #expect(throws: DecodingError.self) {
            try store.addTransactions(
                walletId: walletId,
                transactions: [firstId, invalidId].map { .mock(id: $0, state: .confirmed, assetId: bsv, fee: "42") },
                shouldReplace: { _, _, _ in true },
            )
        }
        #expect(try recordData(db: db, walletId: walletId, transactionId: firstId) == before)
        #expect(throws: DecodingError.self) {
            try store.updateTransaction(
                walletId: walletId, transactionId: invalidId, state: .confirmed,
                fee: nil, blockNumber: nil, metadata: nil, confirmationEtaSeconds: nil,
                shouldReplace: { _, _, _ in true },
            )
        }
    }

    private func recordData(db: DB, walletId: WalletId, transactionId: TransactionId) throws -> Data {
        let record = try #require(try db.dbQueue.read { db in
            try TransactionRecord.filter(TransactionRecord.Columns.walletId == walletId.id)
                .filter(TransactionRecord.Columns.transactionId == transactionId.identifier).fetchOne(db)
        })
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        return try encoder.encode(record)
    }
}
