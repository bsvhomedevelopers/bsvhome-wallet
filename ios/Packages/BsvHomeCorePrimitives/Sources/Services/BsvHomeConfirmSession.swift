// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeConfirmSessionProtocol
import struct BsvHomeCore.BsvHomeConfirmSimulationState
import Primitives

public extension BsvHomeConfirmSessionProtocol {
    var currency: Primitives.Currency {
        Primitives.Currency(core: getCurrency())
    }

    func explorerLink(chain: Primitives.Chain, address: String) -> BlockExplorerLink {
        addressUrl(chain: chain.rawValue, address: address).map()
    }
}

public extension BsvHomeConfirmSimulationState {
    var names: [Primitives.ChainAddress: Primitives.AddressName] {
        Dictionary(
            addressNames
                .map { $0.map() }
                .map { (Primitives.ChainAddress(chain: $0.chain, address: $0.address), $0) },
            uniquingKeysWith: { first, _ in first },
        )
    }
}
