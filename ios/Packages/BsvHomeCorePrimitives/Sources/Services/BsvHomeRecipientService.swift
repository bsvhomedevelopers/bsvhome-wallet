// Copyright (c). Gem Wallet. All rights reserved.

import protocol BsvHomeCore.BsvHomeRecipientServiceProtocol
import Primitives

public extension BsvHomeRecipientServiceProtocol {
    func recipientWallets(wallets: [Wallet]) -> [Wallet] {
        recipientWallets(wallets: wallets.map { $0.map() }).map { $0.map() }
    }
}
