// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.SwapPriceImpactType

public struct PriceImpactValue: Equatable, Sendable {
    let type: SwapPriceImpactType
    let value: String
}
