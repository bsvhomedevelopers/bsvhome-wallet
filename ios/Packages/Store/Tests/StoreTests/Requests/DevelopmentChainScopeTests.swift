import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import GRDB
import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit
import Testing

struct DevelopmentChainScopeTests {
    @Test(arguments: [WalletType.privateKey, .view], [BsvHomeRuntimeEnvironment.production, .localDevelopment])
    func persistedIneligibleWalletsUseCoreVisibility(walletType: WalletType, environment: BsvHomeRuntimeEnvironment) throws {
        let chains: [Chain] = [.tron, .tronShasta]
        let db = DB.mockWithChains(chains)
        let store = WalletStore(db: db)
        let mixed = Wallet.mock(
            id: .make(walletType: walletType, chain: .tron, address: "mainnet"),
            type: walletType,
            accounts: chains.map { .mock(chain: $0) },
        )
        let testOnly = Wallet.mock(
            id: .make(walletType: walletType, chain: .tronShasta, address: "development"),
            type: walletType,
            accounts: [.mock(chain: .tronShasta)],
        )
        try store.addWallet(mixed)
        try store.addWallet(testOnly)

        let visible = try db.dbQueue.read {
            try WalletsRequest(isPinned: nil, allowedChains: chains, environment: environment).fetch($0)
        }
        let selected = try db.dbQueue.read {
            try WalletRequest(walletId: mixed.id, allowedChains: chains, environment: environment).fetch($0)
        }

        #expect(visible.map(\.id) == [mixed.id])
        #expect(visible.first?.accounts.map(\.chain) == [.tron])
        #expect(selected.accounts.map(\.chain) == [.tron])
        #expect(throws: AnyError.self) {
            try db.dbQueue.read {
                try WalletRequest(walletId: testOnly.id, allowedChains: chains, environment: environment).fetch($0)
            }
        }
        #expect(try store.getWallet(id: mixed.id)?.accounts.count == 2)
        #expect(try store.getWallet(id: testOnly.id)?.accounts.map(\.chain) == [.tronShasta])
    }

    @Test
    func persistedTestOnlyWalletsAreHiddenAndMulticoinAccountsAreMasked() throws {
        let db = DB.mockWithChains([.tron, .tronShasta])
        let store = WalletStore(db: db)
        let multicoin = Wallet.mock(accounts: [.mock(chain: .tron), .mock(chain: .tronShasta)])
        let testOnly = Wallet.mock(id: .single(chain: .tronShasta, address: "shasta-sender"), type: .single, accounts: [.mock(chain: .tronShasta)])
        try store.addWallet(multicoin)
        try store.addWallet(testOnly)
        let connections = ConnectionStore(db: db)
        try connections.addConnection(.mock(session: .mock(sessionId: "multicoin-session"), wallet: multicoin))
        try connections.addConnection(.mock(session: .mock(sessionId: "shasta-session"), wallet: testOnly))

        try db.dbQueue.read { (database: Database) throws in
            let production = try WalletsRequest(isPinned: nil, allowedChains: [.tron], environment: .production).fetch(database)
            #expect(production.map(\.id) == [multicoin.id])
            #expect(production.first?.accounts.map(\.chain) == [.tron])
            #expect(throws: AnyError.self) {
                try WalletRequest(walletId: testOnly.id, allowedChains: [.tron], environment: .production).fetch(database)
            }
            let local = try WalletsRequest(isPinned: nil, allowedChains: [.tron, .tronShasta], environment: .localDevelopment).fetch(database)
            #expect(local.count == 2)
            #expect(try WalletRequest(walletId: testOnly.id, allowedChains: [.tronShasta], environment: .localDevelopment).fetch(database).accounts.map(\.chain) == [.tronShasta])
            let productionConnections = try ConnectionsRequest(allowedChains: [.tron], environment: .production).fetch(database)
            #expect(productionConnections.map(\.wallet.id) == [multicoin.id])
            #expect(productionConnections.first?.wallet.accounts.map(\.chain) == [.tron])
            #expect(try ConnectionsRequest(allowedChains: [.tron, .tronShasta], environment: .localDevelopment).fetch(database).count == 2)
        }

        #expect(try store.getWallet(id: multicoin.id)?.accounts.count == 2)
        #expect(try store.getWallet(id: testOnly.id) != nil)
        #expect(try connections.getConnection(sessionId: "shasta-session") != nil)
    }

    @Test
    func storedAssetsSearchAndRecentsRespectAvailability() throws {
        let production = Asset.mock(id: AssetId(chain: .tron), name: "TRON", symbol: "TRX")
        let development = Asset.mock(id: AssetId(chain: .tronShasta), name: "TRON Shasta", symbol: "TRX")
        let db = DB.mockAssets(assets: [.mock(asset: production), .mock(asset: development)])
        let recents = RecentActivityStore(db: db)
        try recents.add(assetId: production.id, toAssetId: nil, walletId: .mock(), type: .transfer)
        try recents.add(assetId: development.id, toAssetId: nil, walletId: .mock(), type: .transfer)

        try db.dbQueue.read { (database: Database) throws in
            var request = AssetsRequest(walletId: .mock(), allowedChains: [.tron], environment: .production)
            #expect(try request.fetch(database).map(\.asset.id) == [production.id])
            request.filters = [.chains([Chain.tronShasta.rawValue])]
            #expect(try request.fetch(database).isEmpty)
            request.filters = []
            #expect(try request.fetch(database).map(\.asset.id) == [production.id])

            #expect(try AssetsRequest(walletId: .mock(), scope: .allAssets, allowedChains: [.tron], environment: .production).fetch(database).map(\.asset.id) == [production.id])
            #expect(try AssetsRequest(walletId: .mock(), allowedChains: [.tron, .tronShasta], environment: .localDevelopment).fetch(database).count == 2)
            #expect(try AssetsRequest(walletId: .mock(), allowedChains: [], environment: .production).fetch(database).isEmpty)
            #expect(try WalletSearchRequest(walletId: .mock(), allowedChains: [.tron], environment: .production).fetch(database).assets.map(\.asset.id) == [production.id])
            #expect(try RecentActivityRequest(walletId: .mock(), allowedChains: [.tron], environment: .production).fetch(database).map(\.asset.id) == [production.id])
            #expect(try RecentActivityRequest(walletId: .mock(), allowedChains: [], environment: .production).fetch(database).isEmpty)
        }
    }

    @Test
    func storedDevelopmentPricesNeverIncreaseFiatTotals() throws {
        let production = Asset.mock(id: AssetId(chain: .tron), symbol: "TRX")
        let development = Asset.mock(id: AssetId(chain: .tronShasta), symbol: "TRX")
        let db = DB.mockAssets(assets: [.mock(asset: production), .mock(asset: development)])
        try BalanceStore(db: db).updateBalances([
            .mock(assetId: production.id, available: 5),
            .mock(assetId: development.id, available: 1_000_000),
        ], for: .mock())
        try PriceStore(db: db).updatePrices([
            .mock(assetId: production.id, price: 2),
            .mock(assetId: development.id, price: 999),
        ])

        try db.dbQueue.read { (database: Database) throws in
            let values = try AssetFiatValuesRequest(walletId: .mock(), priceBasis: .usd, environment: .production).fetch(database)
            #expect(values.count == 1)
            #expect(values.first?.amount == 5)
            #expect(values.first?.price == 2)
            let localAssets = try AssetsRequest(walletId: .mock(), allowedChains: [.tron, .tronShasta], environment: .localDevelopment).fetch(database)
            #expect(localAssets.first { $0.asset.id == production.id }?.price?.price == 2)
            #expect(localAssets.first { $0.asset.id == development.id }?.price == nil)
            #expect(try PriceUsdRequest(assetId: development.id).fetch(database) == nil)
            #expect(try PriceUsdRequest(assetId: production.id).fetch(database) == 2)
        }
    }

    @Test
    func transactionAvailabilityCannotBeRemovedByFilters() throws {
        let productionId = AssetId(chain: .tron)
        let developmentId = AssetId(chain: .tronShasta)
        let db = DB.mockAssets(assets: [.mock(asset: .mock(id: productionId)), .mock(asset: .mock(id: developmentId))])
        try TransactionStore(db: db).addTransactions(walletId: .mock(), transactions: [
            .mock(id: TransactionId(chain: .tron, hash: "mainnet"), state: .pending, assetId: productionId),
            .mock(id: TransactionId(chain: .tronShasta, hash: "development"), state: .pending, assetId: developmentId),
        ])

        try db.dbQueue.read { (database: Database) throws in
            var request = TransactionsRequest(walletId: .mock(), type: .pending, allowedChains: [.tron], environment: .production)
            #expect(try request.fetch(database).map(\.transaction.assetId) == [productionId])
            request.filters = [.chains([Chain.tronShasta.rawValue])]
            #expect(try request.fetch(database).isEmpty)
            request.filters = []
            #expect(try request.fetch(database).count == 1)
            #expect(try TransactionsCountRequest(walletId: .mock(), type: .pending, allowedChains: [.tron], environment: .production).fetch(database) == 1)
            #expect(try TransactionsRequest(walletId: .mock(), type: .pending, allowedChains: [.tron, .tronShasta], environment: .localDevelopment).fetch(database).count == 2)
            #expect(try TransactionsRequest(walletId: .mock(), type: .pending, allowedChains: [], environment: .production).fetch(database).isEmpty)
        }
    }
}
