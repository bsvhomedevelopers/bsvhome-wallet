// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeTransferPasswordError
import Localization

extension BsvHomeTransferPasswordError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .InvalidPassword: Localized.wowpayTransferPasswordInvalid
        case .ConfirmationMismatch: Localized.wowpayTransferPasswordMismatch
        case .CurrentPasswordRequired, .IncorrectPassword: Localized.wowpayTransferPasswordIncorrect
        case .WalletChanged, .InvalidSnapshot: Localized.wowpayTransferPasswordWalletChanged
        case .RateLimited: Localized.wowpayTransferPasswordRateLimited
        case .Cancelled: Localized.Errors.cancelled
        case .WatchOnly: Localized.Wallet.Watch.Tooltip.title
        case .MissingBsvAccount: Localized.Errors.walletAccountMissing
        case .InvalidConfiguration, .NotConfigured, .InvalidResponse, .Unavailable: Localized.wowpayTransferPasswordUnavailable
        }
    }
}
