enum BsvColdWalletScan: String, Identifiable {
    case pairing
    case request
    case signed

    var id: String { rawValue }

    var kind: String {
        switch self {
        case .pairing: "wowpay-bsv-pairing"
        case .request: "wowpay-bsv-request"
        case .signed: "wowpay-bsv-signed"
        }
    }
}
