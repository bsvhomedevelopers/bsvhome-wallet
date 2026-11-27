import Localization

public enum WowPayAssetDetailTab: String, CaseIterable, Identifiable {
    case overview
    case address
    case multisig
    case transactions
    case about

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .overview: Localized.wowpayOverview
        case .address: Localized.Common.address
        case .multisig: Localized.multisigTitle
        case .transactions: Localized.Activity.title
        case .about: Localized.wowpayAbout
        }
    }
}
