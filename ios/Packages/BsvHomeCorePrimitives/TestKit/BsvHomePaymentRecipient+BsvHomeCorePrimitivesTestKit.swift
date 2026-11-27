// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomePaymentRecipient
import struct BsvHomeCore.BsvHomeRecipient

public extension BsvHomePaymentRecipient {
    static func mock(
        recipient: BsvHomeRecipient = .mock(),
        amount: String? = nil,
    ) -> BsvHomePaymentRecipient {
        BsvHomePaymentRecipient(recipient: recipient, amount: amount)
    }
}
