import Foundation
import Primitives

struct BsvPayerJournalMigrationFixture: Decodable {
    let wallet: Wallet
    let storedRevision: UInt64
    let storedValue: String

    static func load() throws -> [Self] {
        guard let url = Bundle.module.url(forResource: "bsv-payer-journal-migration", withExtension: "json") else {
            throw AnyError("Payer journal migration resource is unavailable")
        }
        return try JSONDecoder().decode([Self].self, from: Data(contentsOf: url))
    }
}
