import GRDB
@testable import Store
import StoreTestKit
import Testing

struct PortalSwapStoreTests {
    @Test
    func addsSwapStorageAfterHistoricalMigrationsWithoutLosingData() throws {
        let db = DB.mock()
        try db.dbQueue.write { database in
            try database.drop(table: PortalSwapStateRecord.databaseTableName)
            try database.execute(sql: "DELETE FROM grdb_migrations WHERE identifier = ?", arguments: ["Create portal_swap_states"])
            try database.execute(sql: "CREATE TABLE migration_marker (value TEXT NOT NULL)")
            try database.execute(sql: "INSERT INTO migration_marker VALUES ('preserved')")
            try database.execute(sql: "INSERT INTO grdb_migrations VALUES ('historical migration')")
        }
        var migrations = Migrations()
        try migrations.run(dbQueue: db.dbQueue)
        let store = PortalSwapStore(db: db)
        try store.set(scope: "upgraded-wallet", value: "pending-order")
        #expect(try store.get(scope: "upgraded-wallet") == "pending-order")
        let marker = try db.dbQueue.read { try String.fetchOne($0, sql: "SELECT value FROM migration_marker") }
        #expect(marker == "preserved")
    }

    @Test
    func persistsAcrossStoreInstancesAndKeepsScopesSeparate() throws {
        let db = DB.mock()
        let store = PortalSwapStore(db: db)
        try store.set(scope: "test:wallet-a", value: "pending-operation-a")
        try store.set(scope: "test:wallet-b", value: "pending-operation-b")

        let restored = PortalSwapStore(db: db)
        #expect(try restored.get(scope: "test:wallet-a") == "pending-operation-a")
        #expect(try restored.get(scope: "test:wallet-b") == "pending-operation-b")
        #expect(try restored.get(scope: "production:wallet-a") == nil)

        try restored.set(scope: "test:wallet-a", value: "confirmed-operation-a")
        #expect(try store.get(scope: "test:wallet-a") == "confirmed-operation-a")
        #expect(try store.get(scope: "test:wallet-b") == "pending-operation-b")
    }
}
