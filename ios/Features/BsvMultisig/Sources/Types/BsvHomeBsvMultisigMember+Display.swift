import struct BsvHomeCore.BsvHomeBsvMultisigMember
import Localization

extension BsvHomeBsvMultisigMember {
    var statusTitle: String {
        switch status {
        case .invited: Localized.multisigInvited
        case .awaitingApproval: Localized.multisigAwaitingApproval
        case .confirmed: Localized.multisigConfirmed
        }
    }
}
