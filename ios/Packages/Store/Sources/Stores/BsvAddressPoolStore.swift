import GRDB
import Primitives

public struct BsvAddressPoolStore: Sendable {
    private let db: DatabaseQueue

    public init(db: DB) {
        self.db = db.dbQueue
    }

    public func get(walletId: WalletId) throws -> (revision: UInt64, value: String)? {
        try db.read { database in
            guard let record = try BsvAddressPoolRecord.fetchOne(database, key: walletId.id) else { return nil }
            guard let revision = UInt64(exactly: record.revision) else {
                throw AnyError("BSV address revision is invalid")
            }
            return (revision, record.value)
        }
    }

    public func observeChanges() -> AsyncThrowingStream<Void, any Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let revisions = ValueObservation.tracking { database in
                        let rows = try Row.fetchAll(database, sql: "SELECT walletId, revision FROM bsv_address_pools ORDER BY walletId")
                        return Dictionary(uniqueKeysWithValues: rows.map { row -> (String, Int64) in
                            (row["walletId"], row["revision"])
                        })
                    }
                    for try await _ in revisions.values(in: db) {
                        continuation.yield(())
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    public func save(walletId: WalletId, expectedRevision: UInt64?, revision: UInt64, value: String) throws -> Bool {
        guard let storedRevision = Int64(exactly: revision) else {
            throw AnyError("BSV address revision exceeds storage range")
        }
        if let expectedRevision {
            guard let expected = Int64(exactly: expectedRevision), expected < Int64.max, storedRevision == expected + 1 else {
                throw AnyError("BSV address revision must increase by one")
            }
            return try db.write { database in
                try BsvAddressPoolRecord
                    .filter(Column("walletId") == walletId.id)
                    .filter(Column("revision") == expected)
                    .updateAll(database, [Column("revision").set(to: storedRevision), Column("value").set(to: value)]) == 1
            }
        }
        guard storedRevision == 0 else {
            throw AnyError("BSV address initialization revision must be zero")
        }
        return try db.write { database in
            try BsvAddressPoolRecord(walletId: walletId.id, revision: storedRevision, value: value).insert(database, onConflict: .ignore)
            return database.changesCount == 1
        }
    }

    public func unresolvedTransactions(walletId: WalletId) throws -> [Transaction] {
        try db.read { database in
            try TransactionRecord
                .filter(TransactionRecord.Columns.walletId == walletId.id)
                .filter(TransactionRecord.Columns.chain == Chain.bitcoinSv.rawValue)
                .filter([TransactionDirection.outgoing, .selfTransfer].map(\.rawValue).contains(TransactionRecord.Columns.direction))
                .filter([TransactionState.pending, .inTransit, .failed].map(\.rawValue).contains(TransactionRecord.Columns.state))
                .order(TransactionRecord.Columns.transactionId)
                .fetchAll(database)
                .map { $0.mapToTransaction() }
        }
    }
}
