// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeWalletConnectError
import WalletConnectSign

extension RejectionReason {
    init(from error: Error) {
        if let autoNamespacesError = error as? AutoNamespacesError {
            self = RejectionReason(from: autoNamespacesError)
            return
        }

        guard let walletConnectError = error as? BsvHomeWalletConnectError else {
            self = .userRejected
            return
        }
        self = switch walletConnectError {
        case .UnsupportedChains: .unsupportedChains
        case .UnsupportedWallets: .unsupportedAccounts
        case .InvalidOrigin, .Service: .userRejected
        }
    }
}
