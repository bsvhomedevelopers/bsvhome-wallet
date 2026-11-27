// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeBalanceRequirement
import Primitives

public extension BsvHomeBalanceRequirement {
    func map() -> BalanceRequirement {
        BalanceRequirement(required: required, available: available, shortfall: shortfall)
    }
}
