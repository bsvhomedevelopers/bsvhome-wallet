import Foundation
import enum BsvHomeCore.BsvHomeBsvHdImportMode
import Primitives

struct BsvHdImportPreview: Identifiable, Equatable, Sendable {
    let id: UUID
    let contextID: UUID
    let mode: BsvHomeBsvHdImportMode
    let pairing: String
    let account: Account

    var accountPath: String {
        String(account.derivationPath.dropLast("/0/0".count))
    }
}
