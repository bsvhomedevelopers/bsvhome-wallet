// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeRecipient

public extension BsvHomeRecipient {
    static func mock(
        name: String? = nil,
        address: String = "0x1234567890123456789012345678901234567890",
        memo: String? = nil,
        references: [String] = [],
    ) -> BsvHomeRecipient {
        BsvHomeRecipient(
            address: address,
            name: name,
            memo: memo,
            references: references,
        )
    }
}
