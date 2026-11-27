// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.WalletConnectionVerificationStatus
import ReownWalletKit

extension ReownWalletKit.VerifyContext.ValidationStatus {
    func map() -> WalletConnectionVerificationStatus {
        switch self {
        case .valid: .verified
        case .invalid: .invalid
        case .scam: .malicious
        case .unknown: .unknown
        }
    }
}
