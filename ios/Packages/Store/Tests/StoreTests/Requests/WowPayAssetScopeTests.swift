// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit
import Testing

struct WowPayAssetScopeTests {
    private let visibleAssets: [Asset] = [
        .mock(id: AssetId(chain: .bitcoinSv), name: "Bitcoin SV", symbol: "BSV"),
    ]

    @Test
    func exactAssetsAreFilteredBeforePaginationAndSearchPriority() throws {
        let (db, assets) = try makeDatabase(hiddenTokenCount: 101)
        let visibleIds = Set(visibleAssets.map(\.id))
        #expect(Set(BsvHomeRuntimeEnvironment.wowPay.allowedAssetIds() ?? []) == Set(visibleIds.map(\.identifier)))
        let hiddenIds = assets.map(\.id).filter { !visibleIds.contains($0) }
        try SearchStore(db: db).add(type: .asset, query: "USDT", ids: hiddenIds.map(\.identifier))

        try db.dbQueue.read { database in
            let wallet = try AssetsRequest(walletId: .mock(), limit: 3, environment: .wowPay).fetch(database)
            let allAssets = try AssetsRequest(walletId: .mock(), scope: .allAssets, environment: .wowPay).fetch(database)
            let search = try WalletSearchRequest(walletId: .mock(), limit: 3, environment: .wowPay).fetch(database)
            let usdt = try WalletSearchRequest(walletId: .mock(), searchBy: "USDT", searchKey: "USDT", limit: 1, environment: .wowPay).fetch(database)
            let priority = try AssetsRequest(walletId: .mock(), searchBy: "USDT", limit: 1, environment: .wowPay).fetch(database)

            #expect(Set(wallet.map(\.asset.id)) == visibleIds)
            #expect(Set(allAssets.map(\.asset.id)) == visibleIds)
            #expect(Set(search.assets.map(\.asset.id)) == visibleIds)
            #expect(usdt.assets.isEmpty)
            #expect(priority.isEmpty)
            #expect(search.perpetuals.isEmpty)
            #expect(search.collections.isEmpty)
            #expect(search.lists.isEmpty)

            let unrestricted = try AssetsRequest(walletId: .mock(), scope: .allAssets, environment: .localDevelopment).fetch(database)
            #expect(unrestricted.count == AssetsRequest.defaultQueryLimit)
            #expect(unrestricted.allSatisfy { !visibleIds.contains($0.asset.id) })
        }

        #expect(try Set(AssetStore(db: db).getAssets().map(\.id)) == Set(assets.map(\.id)))
    }

    @Test
    func newerHiddenRecentsCannotReplaceVisibleAssets() throws {
        let (db, assets) = try makeDatabase()
        let recents = RecentActivityStore(db: db)
        let date = Date(timeIntervalSince1970: 1_700_000_000)
        for (index, asset) in assets.enumerated() {
            try recents.add(assetId: asset.id, toAssetId: nil, walletId: .mock(), type: .transfer, createdAt: date.addingTimeInterval(Double(index)))
        }

        try db.dbQueue.read { database in
            let visible = try RecentActivityRequest(walletId: .mock(), limit: 3, environment: .wowPay).fetch(database)
            let unrestricted = try RecentActivityRequest(walletId: .mock(), limit: assets.count, environment: .localDevelopment).fetch(database)

            #expect(visible.map(\.asset.id) == visibleAssets.map(\.id))
            #expect(Set(unrestricted.map(\.asset.id)) == Set(assets.map(\.id)))
        }
    }

    @Test
    func hiddenBalancesAndTransactionsStayStoredAndDoNotAffectTotals() throws {
        let (db, assets) = try makeDatabase()
        let store = TransactionStore(db: db)
        try store.addTransactions(walletId: .mock(), transactions: assets.enumerated().map { index, asset in
            .mock(id: TransactionId(chain: asset.chain, hash: "transaction-\(index)"), state: .pending, assetId: asset.id)
        })

        try db.dbQueue.read { database in
            let values = try AssetFiatValuesRequest(walletId: .mock(), priceBasis: .usd, environment: .wowPay).fetch(database)
            let transactions = try TransactionsRequest(walletId: .mock(), type: .pending, environment: .wowPay).fetch(database)
            let count = try TransactionsCountRequest(walletId: .mock(), type: .pending, environment: .wowPay).fetch(database)
            var widened = AssetsRequest(walletId: .mock(), allowedChains: [.bitcoinSv, .tron, .ethereum, .tronShasta], environment: .wowPay)
            widened.filters = [.chainsOrAssets([], assets.map(\.id.identifier))]

            #expect(values.count == 1)
            #expect(values.reduce(0) { $0 + $1.amount * $1.price } == 2)
            #expect(count == 1)
            #expect(transactions.count == count)
            #expect(Set(transactions.map(\.transaction.assetId)) == Set(visibleAssets.map(\.id)))
            #expect(try Set(widened.fetch(database).map(\.asset.id)) == Set(visibleAssets.map(\.id)))
        }

        #expect(try Set(store.getTransactions(states: [.pending])[.mock(), default: []].map(\.assetId)) == Set(assets.map(\.id)))
        #expect(try BalanceStore(db: db).getBalances(walletId: .mock(), assetIds: assets.map(\.id)).count == assets.count)
        #expect(try Set(WalletStore(db: db).getWallet(id: .mock())?.accounts.map(\.chain) ?? []) == [.bitcoinSv, .tron, .ethereum, .tronShasta])
    }

    @Test
    func assetHistoryLimitsTheNewestScopedRecordsWithoutDeletingOlderHistory() throws {
        let (db, assets) = try makeDatabase()
        let bsv = visibleAssets[0]
        let hidden = try #require(assets.first { $0.chain == .tron })
        let otherWallet = Wallet.mock(id: .multicoin(address: "other-wallet"), accounts: [.mock(chain: .bitcoinSv)])
        try WalletStore(db: db).addWallet(otherWallet)
        let date = Date(timeIntervalSince1970: 1_700_000_000)
        let visible: [Transaction] = (0 ..< 251).map { index in
            .mock(id: TransactionId(chain: .bitcoinSv, hash: "visible-\(index)"), assetId: bsv.id, createdAt: date.addingTimeInterval(Double(index)))
        }
        let hiddenRecords: [Transaction] = (0 ..< 300).map { index in
            .mock(id: TransactionId(chain: hidden.chain, hash: "hidden-\(index)"), assetId: hidden.id, createdAt: date.addingTimeInterval(Double(10000 + index)))
        }
        let filteredRecords: [Transaction] = (0 ..< 220).map { index in
            .mock(id: TransactionId(chain: .bitcoinSv, hash: "filtered-\(index)"), type: .smartContractCall, assetId: bsv.id, createdAt: date.addingTimeInterval(Double(20000 + index)))
        }
        let otherRecords: [Transaction] = (0 ..< 300).map { index in
            .mock(id: TransactionId(chain: .bitcoinSv, hash: "other-\(index)"), assetId: bsv.id, createdAt: date.addingTimeInterval(Double(30000 + index)))
        }
        let store = TransactionStore(db: db)
        let selectedWalletRecords = visible + hiddenRecords + filteredRecords
        try store.addTransactions(walletId: .mock(), transactions: selectedWalletRecords)
        try store.addTransactions(walletId: otherWallet.id, transactions: otherRecords)

        let page = try db.dbQueue.read { database in
            try TransactionsRequest(
                walletId: .mock(),
                type: .asset(assetId: bsv.id),
                filters: [.types([TransactionType.transfer.rawValue])],
                limit: Int(BsvHomeRuntimeEnvironment.wowPay.assetDetailsTransactionsLimit()),
                environment: .wowPay,
            ).fetch(database)
        }
        let expected = Array(visible.suffix(200).reversed())
        #expect(page.count == 200)
        #expect(page.map(\.transaction.id) == expected.map(\.id))
        #expect(page.map(\.transaction.createdAt) == expected.map(\.createdAt))
        let stored = try store.getTransactions(states: [.confirmed])
        #expect(Set(stored[.mock(), default: []].map(\.id)) == Set(selectedWalletRecords.map(\.id)))
        #expect(Set(stored[otherWallet.id, default: []].map(\.id)) == Set(otherRecords.map(\.id)))
    }

    @Test
    func walletAccountsAreProjectedWithoutDeletingStoredNetworks() throws {
        let (db, _) = try makeDatabase()
        let store = WalletStore(db: db)
        let hiddenWallet = Wallet.mock(id: .single(chain: .ethereum, address: "hidden-wallet"), type: .single, accounts: [.mock(chain: .ethereum)])
        try store.addWallet(hiddenWallet)

        try db.dbQueue.read { database in
            let visible = try WalletsRequest(isPinned: nil, environment: .wowPay).fetch(database)
            let wallet = try WalletRequest(walletId: .mock(), environment: .wowPay).fetch(database)
            let production = try WalletRequest(walletId: .mock(), environment: .production).fetch(database)

            #expect(visible.map(\.id) == [.mock()])
            #expect(Set(wallet.accounts.map(\.chain)) == [.bitcoinSv])
            #expect(Set(production.accounts.map(\.chain)) == [.bitcoinSv, .tron, .ethereum])
            #expect(throws: AnyError.self) {
                try WalletRequest(walletId: hiddenWallet.id, environment: .wowPay).fetch(database)
            }
        }

        #expect(try store.getWallet(id: hiddenWallet.id) == hiddenWallet)
        #expect(try store.getWallet(id: .mock())?.accounts.count == 4)
    }

    private func makeDatabase(hiddenTokenCount: Int = 1) throws -> (DB, [Asset]) {
        let hiddenTokens = (0 ..< hiddenTokenCount).map { index in
            Asset.mock(id: AssetId(chain: .tron, tokenId: "hidden-token-\(index)"), name: "Other Tether", symbol: "USDT", decimals: 6, type: .trc20)
        }
        let assets = visibleAssets + hiddenTokens + [
            .mockTron(),
            .mockTronUSDT(),
            .mockEthereumUSDT(),
            .mockEthereum(),
            .mock(id: AssetId(chain: .tronShasta), name: "TRON Shasta", symbol: "TRX", decimals: 6),
        ]
        let visibleIds = Set(visibleAssets.map(\.id))
        let db = DB.mock()
        try AssetStore(db: db).add(assets: assets.map {
            .mock(asset: $0, score: .mock(rank: visibleIds.contains($0.id) ? 1 : 1000))
        })
        let chains: [Chain] = [.bitcoinSv, .tron, .ethereum, .tronShasta]
        try WalletStore(db: db).addWallet(.mock(accounts: chains.map { Account.mock(chain: $0) }))
        let balances = BalanceStore(db: db)
        try balances.addBalance(assetIds: assets.map(\.id), isEnabled: true, for: .mock())
        try balances.updateBalances(assets.map {
            .mock(assetId: $0.id, available: visibleIds.contains($0.id) ? 1 : 1000)
        }, for: .mock())
        try PriceStore(db: db).updatePrices(assets.map {
            .mock(assetId: $0.id, price: visibleIds.contains($0.id) ? 2 : 999)
        })
        return (db, assets)
    }
}
