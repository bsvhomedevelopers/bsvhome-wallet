import Localization

public enum WowPayUnavailablePage {
    case bsv
    case mnee
    case send
    case receive
    case paymail
    case address
    case transactions

    var title: String {
        switch self {
        case .bsv: "BSV"
        case .mnee: "MNEE"
        case .send: Localized.wowpaySendBsv
        case .receive: Localized.wowpayReceiveBsv
        case .paymail: Localized.wowpayPaymail
        case .address: Localized.wowpayAddressManagement
        case .transactions: Localized.Activity.title
        }
    }
}
