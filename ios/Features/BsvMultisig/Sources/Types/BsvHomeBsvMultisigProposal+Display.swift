import struct BsvHomeCore.BsvHomeBsvMultisigProposal
import Localization

extension BsvHomeBsvMultisigProposal {
    var statusTitle: String {
        switch state {
        case .collectingSignatures: Localized.multisigCollecting
        case .readyToBroadcast: Localized.multisigReady
        case .pendingConfirmation: Localized.multisigPendingConfirmation
        case .confirmed: Localized.multisigFinished
        case .stopped: Localized.multisigStopped
        case .expired: Localized.multisigExpired
        case .unknown: Localized.Errors.unknown
        }
    }

    var isPending: Bool { personalPending }
}
