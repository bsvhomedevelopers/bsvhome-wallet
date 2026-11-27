import GRDB

public struct PortalSwapStore: Sendable {
    private let db: DatabaseQueue

    public init(db: DB) {
        self.db = db.dbQueue
    }

    public func get(scope: String) throws -> String? {
        try db.read { database in
            try PortalSwapStateRecord.fetchOne(database, key: scope)?.value
        }
    }

    public func set(scope: String, value: String) throws {
        try db.write { database in
            try PortalSwapStateRecord(scope: scope, value: value).save(database)
        }
    }
}
