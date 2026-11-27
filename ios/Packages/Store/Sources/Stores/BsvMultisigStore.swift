import GRDB
import Primitives

public struct BsvMultisigStore: Sendable {
    private let db: DatabaseQueue

    public init(db: DB) {
        self.db = db.dbQueue
    }

    public func getState() throws -> (revision: UInt64, json: String)? {
        try db.read { database in
            guard let record = try BsvMultisigStateRecord.fetchOne(database, key: 1) else { return nil }
            guard let revision = UInt64(exactly: record.revision) else {
                throw AnyError("BSV multisig revision is invalid")
            }
            return (revision, record.value)
        }
    }

    public func compareAndSwap(expectedRevision: UInt64, json: String) throws -> Bool {
        guard let expected = Int64(exactly: expectedRevision), expected < Int64.max else {
            throw AnyError("BSV multisig revision exceeds storage range")
        }
        return try db.write { database in
            if expected == 0 {
                try BsvMultisigStateRecord(id: 1, revision: 1, value: json).insert(database, onConflict: .ignore)
                return database.changesCount == 1
            }
            return try BsvMultisigStateRecord
                .filter(Column("id") == 1)
                .filter(Column("revision") == expected)
                .updateAll(database, [Column("revision").set(to: expected + 1), Column("value").set(to: json)]) == 1
        }
    }
}
