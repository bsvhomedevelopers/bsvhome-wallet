import GRDB

struct BsvAddressPoolRecord: Codable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "bsv_address_pools"

    var walletId: String
    var revision: Int64
    var value: String
}

extension BsvAddressPoolRecord: CreateTable {
    static func create(db: Database) throws {
        try db.create(table: databaseTableName, ifNotExists: true) {
            $0.column("walletId", .text)
                .primaryKey()
                .notNull()
            $0.column("revision", .integer).notNull().check { $0 >= 0 }
            $0.column("value", .text).notNull()
        }
    }
}
