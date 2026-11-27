import struct BsvHomeCore.BsvHomeBsvMultisigProposal
import struct BsvHomeCore.BsvHomeBsvMultisigWallet

@MainActor
public struct BsvMultisigListContext {
    let wallets: [BsvHomeBsvMultisigWallet]
    let proposals: [BsvHomeBsvMultisigProposal]
    let pendingCount: UInt64
    let onOpenWallet: (BsvHomeBsvMultisigWallet) -> Void
    let onOpenProposal: (BsvHomeBsvMultisigProposal) -> Void
    let isLoading: Bool
    let errorMessage: String?
    let onRefresh: (@MainActor () async -> Void)?
    let pendingWallets: [BsvHomeBsvMultisigWallet]?

    public init(wallets: [BsvHomeBsvMultisigWallet], proposals: [BsvHomeBsvMultisigProposal], pendingCount: UInt64, onOpenWallet: @escaping (BsvHomeBsvMultisigWallet) -> Void, onOpenProposal: @escaping (BsvHomeBsvMultisigProposal) -> Void, isLoading: Bool = false, errorMessage: String? = nil, onRefresh: (@MainActor () async -> Void)? = nil, pendingWallets: [BsvHomeBsvMultisigWallet]? = nil) {
        self.wallets = wallets
        self.proposals = proposals
        self.pendingCount = pendingCount
        self.onOpenWallet = onOpenWallet
        self.onOpenProposal = onOpenProposal
        self.isLoading = isLoading
        self.errorMessage = errorMessage
        self.onRefresh = onRefresh
        self.pendingWallets = pendingWallets
    }
}
