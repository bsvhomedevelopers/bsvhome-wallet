import Foundation

struct BsvColdWalletScanner: Identifiable, Equatable {
    enum Origin: Equatable {
        case initialEntry
        case workspace
    }

    let id: UUID
    let kind: BsvColdWalletScan
    let origin: Origin

    init(id: UUID, kind: BsvColdWalletScan, origin: Origin = .workspace) {
        self.id = id
        self.kind = kind
        self.origin = origin
    }
}
