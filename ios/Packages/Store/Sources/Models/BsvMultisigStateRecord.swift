import GRDB

struct BsvMultisigStateRecord: Codable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "bsv_multisig_state"

    let id: Int
    let revision: Int64
    let value: String
}

extension BsvMultisigStateRecord: CreateTable {
    static func create(db: Database) throws {
        try db.create(table: databaseTableName, ifNotExists: true) {
            $0.column("id", .integer).primaryKey().notNull().check { $0 == 1 }
            $0.column("revision", .integer).notNull().check { $0 > 0 }
            $0.column("value", .text).notNull()
        }
    }
}
