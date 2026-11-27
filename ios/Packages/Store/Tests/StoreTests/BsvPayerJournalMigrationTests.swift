import Foundation
import GRDB
import Primitives
import PrimitivesTestKit
@testable import Store
import StoreTestKit
import Testing

struct BsvPayerJournalMigrationTests {
    private let migrationName = "Retain BSV payer journals"

    @Test
    func legacyFileMigrationPreservesEveryRevisionAndJsonAcrossReopen() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("legacy.sqlite")
        let db = try DB(fileURL: url)
        try installLegacySchema(db)
        let first = Wallet.mock(id: .single(chain: .bitcoinSv, address: "first"), type: .single, accounts: [])
        let second = Wallet.mock(id: .privateKey(chain: .bitcoinSv, address: "second"), type: .privateKey, accounts: [])
        let wallets = WalletStore(db: db)
        try wallets.addWallet(first)
        try wallets.addWallet(second)
        let pools = BsvAddressPoolStore(db: db)
        let firstJson = "{\n  \"public\": [\"raw-a\", \"raw-b\"], \"member\": \"raw-c\"\n}"
        let secondJson = "{\"cancelled\":[\"operation\"],\"signed\":\"public-raw\"}"
        #expect(try pools.save(walletId: first.id, expectedRevision: nil, revision: 0, value: "initial"))
        #expect(try pools.save(walletId: first.id, expectedRevision: 0, revision: 1, value: firstJson))
        #expect(try pools.save(walletId: second.id, expectedRevision: nil, revision: 0, value: secondJson))
        #expect(try db.dbQueue.read { try $0.foreignKeys(on: BsvAddressPoolRecord.databaseTableName).count } == 1)

        let reopened = try DB(fileURL: url)
        let restored = BsvAddressPoolStore(db: reopened)
        #expect(try reopened.dbQueue.read { try $0.foreignKeys(on: BsvAddressPoolRecord.databaseTableName).isEmpty })
        #expect(try restored.get(walletId: first.id)?.revision == 1)
        #expect(try restored.get(walletId: first.id)?.value == firstJson)
        #expect(try restored.get(walletId: second.id)?.revision == 0)
        #expect(try restored.get(walletId: second.id)?.value == secondJson)
        #expect(try WalletStore(db: reopened).getWallet(id: first.id) != nil)
        var migrations = Migrations()
        try migrations.run(dbQueue: reopened.dbQueue)
        #expect(try restored.get(walletId: first.id)?.value == firstJson)
        #expect(try reopened.dbQueue.read { try Int.fetchOne($0, sql: "SELECT COUNT(*) FROM grdb_migrations WHERE identifier = ?", arguments: [migrationName]) } == 1)
    }

    @Test
    func failureAtCommitRollsBackCopiedRowsSchemaAndMigrationMarker() throws {
        let db = DB.mock()
        try installLegacySchema(db)
        let wallet = Wallet.mock(accounts: [])
        try WalletStore(db: db).addWallet(wallet)
        let pools = BsvAddressPoolStore(db: db)
        #expect(try pools.save(walletId: wallet.id, expectedRevision: nil, revision: 0, value: "original-public-journal"))
        let observer = RejectPayerJournalMigrationCommit()
        try db.dbQueue.write { $0.add(transactionObserver: observer, extent: .observerLifetime) }
        var migrations = Migrations()
        #expect(throws: (any Error).self) { try migrations.run(dbQueue: db.dbQueue) }
        #expect(observer.rejectedCommits == 1)
        #expect(try db.dbQueue.read { try $0.foreignKeys(on: BsvAddressPoolRecord.databaseTableName).count } == 1)
        #expect(try db.dbQueue.read { try !$0.tableExists("bsv_address_pools_retained") })
        #expect(try pools.get(walletId: wallet.id)?.value == "original-public-journal")
        #expect(try pools.get(walletId: wallet.id)?.revision == 0)
        #expect(try db.dbQueue.read { try Int.fetchOne($0, sql: "SELECT COUNT(*) FROM grdb_migrations WHERE identifier = ?", arguments: [migrationName]) } == 0)
        try db.dbQueue.write { $0.remove(transactionObserver: observer) }
        var retryMigrations = Migrations()
        try retryMigrations.run(dbQueue: db.dbQueue)
        #expect(try db.dbQueue.read { try $0.foreignKeys(on: BsvAddressPoolRecord.databaseTableName).isEmpty })
        #expect(try pools.get(walletId: wallet.id)?.value == "original-public-journal")
    }

    @Test
    func walletRemovalAndSameIdImportNeverResetThePayerCasRevision() throws {
        let db = DB.mock()
        let wallet = Wallet.mock(accounts: [])
        let wallets = WalletStore(db: db)
        try wallets.addWallet(wallet)
        let pools = BsvAddressPoolStore(db: db)
        #expect(try pools.save(walletId: wallet.id, expectedRevision: nil, revision: 0, value: "unsigned-public-operation"))
        #expect(try pools.save(walletId: wallet.id, expectedRevision: 0, revision: 1, value: "signed-public-raw"))
        try wallets.deleteWallet(for: wallet.id)
        #expect(try wallets.getWallet(id: wallet.id) == nil)
        #expect(try pools.get(walletId: wallet.id)?.revision == 1)
        #expect(try pools.get(walletId: wallet.id)?.value == "signed-public-raw")
        try wallets.addWallet(wallet)
        #expect(try wallets.getWallet(id: wallet.id) != nil)
        #expect(try !pools.save(walletId: wallet.id, expectedRevision: nil, revision: 0, value: "replacement"))
        #expect(try pools.save(walletId: wallet.id, expectedRevision: 1, revision: 2, value: "known-network-result"))
        #expect(try !pools.save(walletId: wallet.id, expectedRevision: 1, revision: 2, value: "late-signed-write"))
        #expect(try pools.get(walletId: wallet.id)?.value == "known-network-result")
        #expect(try pools.get(walletId: wallet.id)?.revision == 2)
    }

    @Test
    func supportedTypedWalletIdsKeepTheActualJournalUnchangedDuringLegacyIdMigration() throws {
        let fixtures = try BsvPayerJournalMigrationFixture.load()
        try #require(fixtures.count == 4)
        for type in [WalletType.single, .privateKey, .view, .multicoin] {
            #expect(fixtures.contains(where: { $0.wallet.type == type }))
        }
        let chains = Array(Set(fixtures.flatMap { $0.wallet.accounts.map(\.chain) }))
        let db = DB.mockWithChains(chains)
        let wallets = WalletStore(db: db)
        let pools = BsvAddressPoolStore(db: db)
        for fixture in fixtures {
            try wallets.addWallet(fixture.wallet)
            let revision = try #require(Int64(exactly: fixture.storedRevision))
            try db.dbQueue.write {
                try BsvAddressPoolRecord(walletId: fixture.wallet.id.id, revision: revision, value: fixture.storedValue).insert($0)
            }
        }
        let defaults = UserDefaults.mock()
        for _ in 0 ..< 2 {
            try db.dbQueue.write { try WalletIdMigration.migrate(db: $0, userDefaults: defaults) }
            for fixture in fixtures {
                let wallet = try #require(try wallets.getWallet(id: fixture.wallet.id))
                #expect(wallet.id == fixture.wallet.id)
                #expect(wallet.externalId == fixture.wallet.externalId)
                #expect(wallet.accounts.sorted { $0.chain.rawValue < $1.chain.rawValue } == fixture.wallet.accounts.sorted { $0.chain.rawValue < $1.chain.rawValue })
                #expect(try pools.get(walletId: fixture.wallet.id)?.revision == fixture.storedRevision)
                #expect(try pools.get(walletId: fixture.wallet.id)?.value == fixture.storedValue)
            }
            #expect(try db.dbQueue.read { try Int.fetchOne($0, sql: "SELECT COUNT(*) FROM bsv_address_pools") } == fixtures.count)
        }
    }

    private func installLegacySchema(_ db: DB) throws {
        try db.dbQueue.write {
            try $0.drop(table: BsvAddressPoolRecord.databaseTableName)
            try $0.execute(sql: """
            CREATE TABLE bsv_address_pools (
                walletId TEXT NOT NULL PRIMARY KEY REFERENCES wallets(id) ON DELETE CASCADE ON UPDATE CASCADE,
                revision INTEGER NOT NULL CHECK (revision >= 0),
                value TEXT NOT NULL
            )
            """)
            try $0.execute(sql: "DELETE FROM grdb_migrations WHERE identifier = ?", arguments: [migrationName])
        }
    }
}
