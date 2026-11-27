import enum BsvHomeCore.BsvHomePortalSwapActionType
import enum BsvHomeCore.BsvHomePortalSwapOrderStatus
import enum BsvHomeCore.BsvHomePortalSwapResolutionStatus
import enum BsvHomeCore.BsvHomePortalSwapSyncStatus
import enum BsvHomeCore.BsvHomePortalSwapVerificationStatus
import Localization

enum WowPayPortalOrderLabels {
    static func phase(_ value: BsvHomePortalSwapOrderStatus) -> String {
        switch value {
        case .creating: Localized.wowpaySwapStateCreating
        case .createUnknown: Localized.wowpaySwapStateCreateUnknown
        case .awaitingSignature: Localized.wowpaySwapStateAwaitingSignature
        case .awaitingDeposit: Localized.wowpaySwapStateAwaitingDeposit
        case .confirming: Localized.wowpaySwapStateConfirming
        case .exchanging, .sending: Localized.wowpaySwapStateProcessing
        case .onHold: Localized.wowpaySwapStateOnHold
        case .completed: Localized.Transaction.Status.completed
        case .expired: Localized.wowpaySwapStateExpired
        case .failed: Localized.Transaction.Status.failed
        case .refundPending, .refunding: Localized.wowpaySwapStateRefunding
        case .refunded: Localized.Transaction.Status.refunded
        }
    }

    static func verification(_ value: BsvHomePortalSwapVerificationStatus) -> String? {
        switch value {
        case .notRequired: nil
        case .required: Localized.wowpaySwapVerificationRequired
        case .inProgress: Localized.wowpaySwapVerificationInProgress
        case .underReview: Localized.wowpaySwapVerificationUnderReview
        case .unknown: Localized.Errors.unknown
        case .additionalInfoRequired: Localized.wowpaySwapVerificationAdditionalInfo
        case .approved: Localized.wowpaySwapVerificationApproved
        case .rejected: Localized.wowpaySwapVerificationRejected
        case .expired: Localized.wowpaySwapStateExpired
        }
    }

    static func synchronization(_ value: BsvHomePortalSwapSyncStatus) -> String? {
        switch value {
        case .fresh: nil
        case .stale, .reconciling: Localized.wowpaySwapStateUpdating
        }
    }

    static func settlement(_ value: BsvHomePortalSwapResolutionStatus) -> String {
        switch value {
        case .open: Localized.Transaction.Status.inprogress
        case .settled: Localized.Transaction.Status.completed
        case .needsReview: Localized.wowpaySwapStateNeedsReview
        }
    }

    static func action(_ value: BsvHomePortalSwapActionType) -> String {
        switch value {
        case .verifyIdentity: Localized.wowpaySwapActionVerify
        case .continueVerification: Localized.wowpaySwapVerificationInProgress
        case .submitAdditionalInfo: Localized.wowpaySwapActionAdditionalInfo
        case .contactSupport: Localized.wowpaySwapActionSupport
        }
    }
}
