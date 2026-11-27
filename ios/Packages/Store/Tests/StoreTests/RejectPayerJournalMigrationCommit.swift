import Foundation
import GRDB

final class RejectPayerJournalMigrationCommit: TransactionObserver, @unchecked Sendable {
    private var copiedRows = false
    private(set) var rejectedCommits = 0

    func observes(eventsOfKind eventKind: DatabaseEventKind) -> Bool {
        eventKind.tableName == "bsv_address_pools_retained"
    }

    func databaseDidChange(with _: DatabaseEvent) {
        copiedRows = true
    }

    func databaseWillCommit() throws {
        if copiedRows {
            rejectedCommits += 1
            throw NSError(domain: "PayerJournalMigrationTest", code: 1)
        }
    }

    func databaseDidCommit(_: Database) {}

    func databaseDidRollback(_: Database) {
        copiedRows = false
    }
}
