// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.PerpetualModifyConfirmData
import enum BsvHomeCore.PerpetualModifyPositionType
import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit

public extension PerpetualModifyConfirmData {
    static func mock(
        baseAsset: Asset = .mock(),
        assetIndex: Int32 = 0,
        modifyTypes: [PerpetualModifyPositionType] = [],
        takeProfitOrderId: UInt64? = nil,
        stopLossOrderId: UInt64? = nil,
    ) -> PerpetualModifyConfirmData {
        PerpetualModifyConfirmData(
            baseAsset: baseAsset.map(),
            assetIndex: assetIndex,
            modifyTypes: modifyTypes,
            takeProfitOrderId: takeProfitOrderId,
            stopLossOrderId: stopLossOrderId,
        )
    }
}
