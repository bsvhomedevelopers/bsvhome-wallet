import GRDB
import Primitives
import PrimitivesTestKit
@testable import Store
import StoreTestKit
import Testing

struct BsvAddressPoolStoreTests {
    @Test
    func observesAddressPoolRevisionsWhileWalletDeletionRetainsTheJournal() async throws {
        let db = DB.mockWithChains([.bitcoinSv])
        let wallet = Wallet.mock(accounts: [.mock(chain: .bitcoinSv)])
        let wallets = WalletStore(db: db)
        try wallets.addWallet(wallet)
        let pools = BsvAddressPoolStore(db: db)
        var changes = pools.observeChanges().makeAsyncIterator()

        _ = try #require(try await changes.next())
        #expect(try pools.save(walletId: wallet.id, expectedRevision: nil, revision: 0, value: "initial-public-pool"))
        _ = try #require(try await changes.next())
        #expect(try pools.save(walletId: wallet.id, expectedRevision: 0, revision: 1, value: "issued-public-address"))
        _ = try #require(try await changes.next())
        try wallets.deleteWallet(for: wallet.id)
        #expect(try wallets.getWallet(id: wallet.id) == nil)
        #expect(try pools.get(walletId: wallet.id)?.revision == 1)
        #expect(try pools.get(walletId: wallet.id)?.value == "issued-public-address")
        #expect(try pools.save(walletId: wallet.id, expectedRevision: 1, revision: 2, value: "retained-checkpoint"))
        _ = try #require(try await changes.next())
    }

    @Test
    func savesOnlyTheMatchingRevision() throws {
        let db = DB.mockWithChains([.bitcoinSv])
        let wallet = Wallet.mock(accounts: [.mock(chain: .bitcoinSv, address: "main-address")])
        try WalletStore(db: db).addWallet(wallet)
        let store = BsvAddressPoolStore(db: db)

        #expect(try store.save(walletId: wallet.id, expectedRevision: nil, revision: 0, value: "initial"))
        #expect(try !store.save(walletId: wallet.id, expectedRevision: nil, revision: 0, value: "duplicate"))
        #expect(try store.save(walletId: wallet.id, expectedRevision: 0, revision: 1, value: "updated"))
        #expect(try !store.save(walletId: wallet.id, expectedRevision: 0, revision: 1, value: "stale"))

        let restored = try #require(try BsvAddressPoolStore(db: db).get(walletId: wallet.id))
        #expect(restored.revision == 1)
        #expect(restored.value == "updated")
    }

    @Test
    func rejectsInvalidRevisionBeforeChangingState() throws {
        let db = DB.mockWithChains([.bitcoinSv])
        let wallet = Wallet.mock(accounts: [.mock(chain: .bitcoinSv)])
        try WalletStore(db: db).addWallet(wallet, bsvAddresses: (0, "initial"))
        let store = BsvAddressPoolStore(db: db)

        #expect(throws: (any Error).self) {
            try store.save(walletId: wallet.id, expectedRevision: 0, revision: 2, value: "skipped")
        }
        #expect(throws: (any Error).self) {
            try store.save(walletId: wallet.id, expectedRevision: nil, revision: UInt64.max, value: "overflow")
        }
        #expect(try store.get(walletId: wallet.id)?.value == "initial")
    }

    @Test
    func initializationIsAtomicAndKeepsOneAccount() throws {
        let db = DB.mockWithChains([.bitcoinSv])
        let wallet = Wallet.mock(accounts: [.mock(chain: .bitcoinSv, address: "main-address")])
        let wallets = WalletStore(db: db)
        let pools = BsvAddressPoolStore(db: db)
        try db.dbQueue.write {
            try $0.execute(sql: "CREATE TRIGGER reject_bsv_pool BEFORE INSERT ON bsv_address_pools BEGIN SELECT RAISE(ABORT, 'rejected'); END")
        }

        #expect(throws: (any Error).self) {
            try wallets.addWallet(wallet, bsvAddresses: (0, "ten-public-addresses"))
        }
        #expect(try wallets.getWallet(id: wallet.id) == nil)
        #expect(try pools.get(walletId: wallet.id) == nil)

        try db.dbQueue.write { try $0.execute(sql: "DROP TRIGGER reject_bsv_pool") }
        try wallets.addWallet(wallet, bsvAddresses: (0, "ten-public-addresses"))
        #expect(try wallets.getWallet(id: wallet.id)?.accounts == wallet.accounts)
        #expect(try pools.get(walletId: wallet.id)?.value == "ten-public-addresses")

        #expect(throws: (any Error).self) {
            try wallets.addWallet(wallet, bsvAddresses: (0, "replacement"))
        }
        #expect(try pools.get(walletId: wallet.id)?.value == "ten-public-addresses")
        try wallets.deleteWallet(for: wallet.id)
        #expect(try pools.get(walletId: wallet.id)?.value == "ten-public-addresses")
        #expect(try pools.get(walletId: wallet.id)?.revision == 0)
    }

    @Test
    func addsPoolTableWithoutChangingHistoricalWallet() throws {
        let db = DB.mockWithChains([.bitcoinSv])
        let wallet = Wallet.mock(name: "Existing wallet", accounts: [.mock(chain: .bitcoinSv, address: "unchanged")])
        let wallets = WalletStore(db: db)
        try wallets.addWallet(wallet)
        try db.dbQueue.write { database in
            try database.drop(table: BsvAddressPoolRecord.databaseTableName)
            try database.execute(sql: "DELETE FROM grdb_migrations WHERE identifier = ?", arguments: ["Create bsv_address_pools"])
            try database.execute(sql: "INSERT INTO grdb_migrations VALUES ('historical migration')")
        }

        var migrations = Migrations()
        try migrations.run(dbQueue: db.dbQueue)
        #expect(try wallets.getWallet(id: wallet.id)?.accounts == wallet.accounts)
        #expect(try wallets.getWallet(id: wallet.id)?.name == wallet.name)
        #expect(try BsvAddressPoolStore(db: db).get(walletId: wallet.id) == nil)
    }

    @Test
    func unresolvedTransactionsAreWalletAndChainScoped() throws {
        let db = DB.mockWithChains([.bitcoinSv, .ethereum])
        let first = Wallet.mock(id: .multicoin(address: "first"))
        let second = Wallet.mock(id: .multicoin(address: "second"))
        let wallets = WalletStore(db: db)
        try wallets.addWallet(first)
        try wallets.addWallet(second)
        let transactions = TransactionStore(db: db)
        let pending = Transaction.mock(id: TransactionId(chain: .bitcoinSv, hash: "pending"), state: .pending, direction: .outgoing, assetId: Chain.bitcoinSv.assetId)
        let failed = Transaction.mock(id: TransactionId(chain: .bitcoinSv, hash: "failed"), state: .failed, direction: .selfTransfer, assetId: Chain.bitcoinSv.assetId)
        let confirmed = Transaction.mock(id: TransactionId(chain: .bitcoinSv, hash: "confirmed"), state: .confirmed, direction: .outgoing, assetId: Chain.bitcoinSv.assetId)
        let incoming = Transaction.mock(id: TransactionId(chain: .bitcoinSv, hash: "incoming"), state: .pending, direction: .incoming, assetId: Chain.bitcoinSv.assetId)
        let ethereum = Transaction.mock(id: TransactionId(chain: .ethereum, hash: "ethereum"), state: .pending, direction: .outgoing, assetId: Chain.ethereum.assetId)
        try transactions.addTransactions(walletId: first.id, transactions: [pending, failed, confirmed, incoming, ethereum])
        try transactions.addTransactions(walletId: second.id, transactions: [pending])

        let unresolved = try BsvAddressPoolStore(db: db).unresolvedTransactions(walletId: first.id)
        #expect(Set(unresolved.map(\.id)) == [pending.id, failed.id])
        #expect(try BsvAddressPoolStore(db: db).unresolvedTransactions(walletId: second.id).map(\.id) == [pending.id])
    }
}
