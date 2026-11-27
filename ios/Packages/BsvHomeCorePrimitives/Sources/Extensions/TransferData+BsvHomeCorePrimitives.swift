// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.TransactionInputType
import struct BsvHomeCore.BsvHomeTransferData
import Primitives

public extension BsvHomeTransferData {
    var asset: Primitives.Asset {
        inputAsset().map()
    }

    var chain: Chain {
        asset.chain
    }

    var applicationMetadata: Primitives.ApplicationMetadata? {
        guard case let .generic(_, metadata, _) = inputType else { return nil }
        return metadata.map()
    }

    var id: String {
        identifier()
    }
}
