import enum BsvHomeCore.BsvHomeBsvDiscoveryState
import Localization

public extension BsvHomeBsvDiscoveryState {
    var statusTitle: String {
        switch self {
        case .notStarted: Localized.wowpayBsvScanNotStarted
        case .incomplete: Localized.wowpayBsvScanIncomplete
        case .complete: Localized.wowpayBsvScanComplete
        }
    }
}
