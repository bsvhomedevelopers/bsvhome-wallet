import Primitives

enum WalletPresentation: Codable, Equatable, Sendable {
    case local
    case collective(signerWalletId: WalletId, walletId: String)
    case collectiveOverview(signerWalletId: WalletId)
    case mpc(walletId: String)

    var persistentSelection: Self? {
        switch self {
        case let .collective(_, walletId) where !walletId.isEmpty: self
        case .collectiveOverview: self
        case .local, .collective, .mpc: nil
        }
    }

    var signerWalletId: WalletId? {
        switch self {
        case let .collective(id, _), let .collectiveOverview(id): id
        case .local, .mpc: nil
        }
    }
}
