import Foundation
import enum BsvHomeCore.BsvHomePaymailError
import Localization

extension BsvHomePaymailError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case let .Auth(error):
            switch error {
            case .WatchOnly: Localized.Wallet.Watch.Tooltip.title
            case .MissingBsvAccount: Localized.Errors.walletAccountMissing
            case .WalletChanged: Localized.paymailBindingChanged
            case .InvalidChallenge: Localized.paymailSignatureRejected
            case .InvalidSession, .InvalidResponse: Localized.paymailInvalidResponse
            case let .Api(code, _): Self.apiMessage(code)
            case let .Service(error): error.localizedDescription
            }
        case .InvalidAlias, .InvalidDomain, .InvalidHandle: Localized.Errors.invalidAddressName
        case .InvalidConfiguration: Localized.paymailUnavailable
        case .UnsupportedCapability, .UnsupportedOutput: Localized.Errors.notSupported
        case .InvalidChallenge: Localized.paymailSignatureRejected
        case .InvalidResponse: Localized.paymailInvalidResponse
        case .IdentityMismatch: Localized.paymailIdentityMismatch
        case .BindingChanged: Localized.paymailBindingChanged
        case .PendingVerification: Localized.paymailPendingVerification
        case .InvalidAmount: Localized.Errors.invalidAmount
        case .SenderRequired: Localized.wowpayPaymailSenderRequired
        case let .Api(code, _): Self.apiMessage(code)
        }
    }

    private static func apiMessage(_ code: String) -> String {
        switch code {
        case "PAYMAIL_DISABLED", "PAYMAIL_SERVER_ERROR": Localized.paymailUnavailable
        case "ALIAS_RESERVED", "ALIAS_UNAVAILABLE", "PAYMAIL_NOT_FOUND": Localized.paymailNameUnavailable
        case "DOMAIN_ALREADY_REGISTERED", "ALIAS_OR_DOMAIN_UNAVAILABLE", "BINDING_CHANGED", "BINDING_NOT_REGISTERED", "DOMAIN_NOT_HOSTED": Localized.paymailBindingChanged
        case "IDENTITY_KEY_IMMUTABLE", "SEPARATE_IDENTITY_KEY_REQUIRED", "INVALID_IDENTITY_KEY": Localized.paymailIdentityMismatch
        case "INVALID_SIGNATURE", "CHALLENGE_EXPIRED_OR_USED", "WALLET_AUTH_SIGNATURE_INVALID", "WALLET_AUTH_CHALLENGE_EXPIRED", "WALLET_AUTH_ADDRESS_MISMATCH": Localized.paymailSignatureRejected
        case "INVALID_ALIAS", "INVALID_DOMAIN", "INVALID_HANDLE", "INVALID_BSV_ADDRESS": Localized.Errors.invalidAddressName
        case "RATE_LIMITED", "HTTP_429": Localized.paymailRateLimited
        case "WALLET_AUTH_CHAIN_UNSUPPORTED": Localized.Errors.notSupported
        default: "\(Localized.paymailInvalidResponse) (\(code))"
        }
    }
}
