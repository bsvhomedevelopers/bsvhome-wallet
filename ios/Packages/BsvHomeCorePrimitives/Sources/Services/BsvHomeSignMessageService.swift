// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeSignMessagePreview
import protocol BsvHomeCore.BsvHomeSignMessageServiceProtocol
import Primitives

public extension BsvHomeSignMessageServiceProtocol {
    func addressNames(chain: Chain, preview: BsvHomeSignMessagePreview) async -> [ChainAddress: AddressName] {
        let names = await addressNames(chain: chain.rawValue, preview: preview).map { $0.map() }
        return Dictionary(uniqueKeysWithValues: names.map { (ChainAddress(chain: $0.chain, address: $0.address), $0) })
    }
}
