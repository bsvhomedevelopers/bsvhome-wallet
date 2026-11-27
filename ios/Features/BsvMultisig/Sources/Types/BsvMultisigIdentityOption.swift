public struct BsvMultisigIdentityOption: Identifiable, Hashable, Sendable {
    public let walletId: String
    public let name: String
    public let address: String
    public let isDedicated: Bool

    public var id: String { walletId }

    public init(walletId: String, name: String, address: String, isDedicated: Bool) {
        self.walletId = walletId
        self.name = name
        self.address = address
        self.isDedicated = isDedicated
    }
}
