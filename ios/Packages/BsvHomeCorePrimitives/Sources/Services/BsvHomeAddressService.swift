// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeAddressFormatStyle
import protocol BsvHomeCore.BsvHomeAddressServiceProtocol
import Primitives

public extension BsvHomeAddressServiceProtocol {
    func format(address: String, chain: Primitives.Chain?, style: BsvHomeAddressFormatStyle = .short) -> String {
        format(address: address, chain: chain?.map(), style: style)
    }
}
