import GRDB

struct PortalSwapStateRecord: Codable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "portal_swap_states"

    var scope: String
    var value: String
}

extension PortalSwapStateRecord: CreateTable {
    static func create(db: Database) throws {
        try db.create(table: databaseTableName, ifNotExists: true) {
            $0.column("scope", .text).primaryKey()
            $0.column("value", .text).notNull()
        }
    }
}
