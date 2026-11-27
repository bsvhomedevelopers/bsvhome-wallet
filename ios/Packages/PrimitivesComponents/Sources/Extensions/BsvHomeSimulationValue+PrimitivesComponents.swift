// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeSimulationValue
import BsvHomeCorePrimitives

public extension BsvHomeSimulationValue {
    func map() -> AssetValueHeaderData {
        AssetValueHeaderData(asset: asset.map(), value: value.map())
    }
}
