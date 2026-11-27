// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.PerpetualConfirmData
import enum BsvHomeCore.PerpetualDirection
import struct BsvHomeCore.PerpetualReduceData

public extension PerpetualReduceData {
    static func mock(
        data: PerpetualConfirmData = .mock(),
        positionDirection: PerpetualDirection = .long,
    ) -> PerpetualReduceData {
        PerpetualReduceData(
            data: data,
            positionDirection: positionDirection,
        )
    }
}
