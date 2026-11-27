actor MpcWalletMockCalls {
    private(set) var signatures = 0
    private(set) var preparations = 0
    private(set) var broadcasts = 0
    func prepare() {
        preparations += 1
    }

    func sign() {
        signatures += 1
    }

    func broadcast() {
        broadcasts += 1
    }
}
