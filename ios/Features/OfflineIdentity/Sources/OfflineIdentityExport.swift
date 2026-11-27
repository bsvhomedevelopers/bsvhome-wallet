import Foundation

struct OfflineIdentityExport: Sendable {
    enum Kind: Sendable {
        case backup(fingerprint: String)
        case publicKey
        case proof
    }

    let kind: Kind
    let text: String
    let filename: String
    let generation: UInt64
}
