import struct BsvHomeCore.BsvHomeBsvMultisigWallet
import Localization

extension BsvHomeBsvMultisigWallet {
    var rule: String { "\(threshold)/\(memberCount)" }
    public var statusTitle: String {
        switch status {
        case .waitingForMembers: Localized.multisigWaitingMembers
        case .waitingForConfirmations: Localized.multisigWaitingConfirmations
        case .active: Localized.multisigActive
        case .unknown: Localized.Errors.unknown
        }
    }

    var configurationStatusTitle: String {
        switch configurationStatus {
        case .unavailable: statusTitle
        case .needsConfirmation: Localized.multisigConfigurationNotice
        case .needsLocalReview: Localized.multisigLocalReviewNotice
        case .waitingForMembers: Localized.multisigWaitingConfirmations
        case .confirmed: Localized.multisigConfirmed
        case .changed: Localized.multisigConfigurationChanged
        }
    }
}
