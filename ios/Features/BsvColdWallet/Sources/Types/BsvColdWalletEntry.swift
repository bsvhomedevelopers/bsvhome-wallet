public enum BsvColdWalletEntry: Sendable, Equatable {
    public enum ScanRequestPresentation: Sendable, Equatable {
        case initial
        case resumed
    }

    case home
    case scanRequest(presentation: ScanRequestPresentation)
    case create
    case recover
    case connect
    case signing
    case pairing
    case send
    case receive
}
