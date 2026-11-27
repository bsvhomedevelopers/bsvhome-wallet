// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomePerpetualTransferData
import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit

public extension BsvHomePerpetualTransferData {
    static func mock(
        direction: PerpetualDirection = .long,
        asset: Asset = .mock(),
        leverage: UInt8 = 3,
    ) -> BsvHomePerpetualTransferData {
        BsvHomePerpetualTransferData(
            provider: PerpetualProvider.hypercore.map(),
            direction: direction.map(),
            asset: asset.map(),
            baseAsset: Asset.mock().map(),
            assetIndex: 0,
            price: 100.0,
            leverage: leverage,
            marginType: PerpetualMarginType.cross.map(),
        )
    }
}
