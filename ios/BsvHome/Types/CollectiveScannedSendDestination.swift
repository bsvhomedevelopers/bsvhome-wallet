import Foundation

struct CollectiveScannedSendDestination: Hashable, Codable {
    let id: UUID
    let signerWalletId: String
    let walletId: String
    let code: String

    init(signerWalletId: String, walletId: String, code: String) {
        id = UUID()
        self.signerWalletId = signerWalletId
        self.walletId = walletId
        self.code = code
    }
}
