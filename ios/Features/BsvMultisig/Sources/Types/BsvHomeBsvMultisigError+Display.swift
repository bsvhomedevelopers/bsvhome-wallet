import enum BsvHomeCore.BsvHomeBsvMultisigError
import Localization

extension BsvHomeBsvMultisigError {
    static func displayedMessage(for error: any Error) -> String? {
        guard let error = error as? Self else { return Localized.Errors.errorOccurred }
        if case .Cancelled = error {
            return nil
        }
        return error.localizedMessage
    }

    var localizedMessage: String {
        switch self {
        case let .InvalidPackage(reason):
            if ["INVALID_REQUEST", "INVALID_RESPONSE", "INVALID_AMOUNT", "INVALID_RECIPIENT", "INVALID_NOTE", "INVALID_INVITE", "INVALID_MEMBER_SIGNATURE", "INVALID_BSV_TRANSACTION", "IDEMPOTENCY_CONFLICT", "BROADCAST_CONFLICT", "CREATOR_REQUIRED", "INVITATION_CLOSED", "INVITATION_WALLET_MISMATCH", "INVALID_INVITE_REVIEW", "PROPOSAL_CLOSED", "PROPOSAL_NOT_READY", "WALLET_LIST_LIMIT", "PROPOSAL_HISTORY_LIMIT", "MULTISIG_REQUEST_TOO_LARGE", "INSUFFICIENT_FUNDS", "INPUT_ALREADY_SPENT", "PAYMAIL_SENDER_REQUIRED"].contains(reason) {
                BsvHomeBsvMultisigError.Api(code: reason).localizedMessage
            } else {
                Localized.multisigServiceError("INVALID_BSV_TRANSACTION")
            }
        case .InvalidRules: Localized.multisigRuleLimits
        case .ConfigurationConflict, .FundingChanged: Localized.multisigConfigurationChanged
        case .ConcurrentUpdate: Localized.multisigErrorConcurrentUpdate
        case .MissingSigner, .FundingPayerMissing: Localized.multisigErrorMissingSigner
        case .NotActivated: Localized.multisigErrorNotActive
        case .MissingRecord: Localized.multisigErrorMissingRecord
        case .Provider: Localized.multisigErrorProvider
        case .Storage: Localized.multisigErrorStorage
        case .Authentication: Localized.multisigErrorAuthentication
        case .ConfigurationNotConfirmed: Localized.multisigLocalReviewNotice
        case .WalletChanged: Localized.multisigErrorConcurrentUpdate
        case .WatchOnly: Localized.Wallet.Watch.Tooltip.title
        case .FundingExpired: Localized.multisigFundingExpired
        case .FundingPending: Localized.multisigFundingPendingNotice
        case .CreationUncertain: Localized.multisigUncertain
        case .Cancelled: Localized.Common.cancel
        case let .Api(code):
            switch code {
            case "STALE_VERSION", "SIGNATURE_CONFLICT", "FINAL_TRANSACTION_MISMATCH", "BROADCAST_CONFLICT": Localized.multisigErrorConcurrentUpdate
            case "CONFIGURATION_MISMATCH", "CONFIGURATION_CHANGED": Localized.multisigConfigurationChanged
            case "LOGIN_REQUIRED", "HTTP_401": Localized.multisigErrorAuthentication
            case "RATE_LIMITED", "MULTISIG_RATE_LIMITED", "HTTP_429": Localized.paymailRateLimited
            case "BSV_IDENTITY_REQUIRED", "NOT_A_MEMBER", "SIGNER_IDENTITY_MISMATCH": Localized.multisigErrorMissingSigner
            case "INVALID_INVITE", "INVITE_EXPIRED", "INVITE_USED", "INVALID_INVITE_REVIEW", "INVITATION_WALLET_MISMATCH": Localized.multisigInvalidInvite
            case "INVALID_AMOUNT": Localized.Errors.invalidAmount
            case "INVALID_RECIPIENT": Localized.Errors.invalidAddressName
            case "PAYMAIL_SENDER_REQUIRED": Localized.wowpayPaymailSenderRequired
            case "INVALID_MULTISIG_PAYMENT": Localized.multisigPaymentInvalid
            case "INVALID_ALIAS": Localized.multisigPaymailInvalidName
            case "ALIAS_UNAVAILABLE", "ALIAS_RESERVED": Localized.paymailNameUnavailable
            case "BINDING_CHANGED": Localized.multisigErrorConcurrentUpdate
            case "INSUFFICIENT_FUNDS": Localized.Transfer.insufficientBalance("BSV")
            case "WALLET_NOT_ACTIVE", "PROPOSAL_NOT_READY", "PROPOSAL_CLOSED": Localized.multisigErrorNotActive
            case "WALLET_NOT_FOUND", "PROPOSAL_NOT_FOUND": Localized.multisigErrorMissingRecord
            case "DUPLICATE_POLICY_EXISTING_WALLET": Localized.multisigErrorConfigurationConflict
            case "BSV_BROADCAST_UNCONFIRMED": Localized.multisigFundingPendingNotice
            case "BSV_PROVIDER_UNAVAILABLE", "BSV_PROVIDER_INCOMPLETE", "BSV_PROVIDER_INCONSISTENT", "MULTISIG_SERVICE_UNAVAILABLE": Localized.multisigErrorProvider
            default: Localized.Errors.errorOccurred
            }
        }
    }
}
