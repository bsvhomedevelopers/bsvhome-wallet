public struct WowPayMpcError: Error, Sendable, Equatable {
    public let code: String

    public init(code: String) {
        self.code = code
    }
}
