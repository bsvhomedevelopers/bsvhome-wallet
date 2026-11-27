import Foundation

struct OfflineIdentityPublicState: Sendable {
    let publicKey: String
    let fingerprint: String
    let backupSaved: Bool
}
