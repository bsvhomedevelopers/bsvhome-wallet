// Copyright (c). Gem Wallet. All rights reserved.

import Primitives

public enum WalletOnboardingPurpose: Sendable {
    case personal
    case dedicatedMultisigIdentity(onComplete: @MainActor @Sendable (Wallet) -> Void)

    var isDedicatedMultisigIdentity: Bool {
        switch self {
        case .personal: false
        case .dedicatedMultisigIdentity: true
        }
    }
}
