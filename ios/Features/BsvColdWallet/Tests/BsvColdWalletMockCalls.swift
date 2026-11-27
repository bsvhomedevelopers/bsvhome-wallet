actor BsvColdWalletMockCalls {
    private(set) var pairings: [String] = []
    private(set) var pairingGenerations: [UInt64] = []
    func exportedPairing(walletId: String, generation: UInt64) {
        pairings.append(walletId)
        pairingGenerations.append(generation)
    }

    private(set) var creates = 0
    private(set) var protections = 0
    func created() {
        creates += 1
    }

    func protected() {
        protections += 1
    }
}
