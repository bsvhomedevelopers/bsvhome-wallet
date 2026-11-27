// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import struct BsvHomeCore.BsvHomeSwapRate

public struct AssetRateViewModel {
    private let rate: BsvHomeSwapRate
    private let formatter: NumericFormatter

    public init(rate: BsvHomeSwapRate, formatter: NumericFormatter = NumericFormatter()) {
        self.rate = rate
        self.formatter = formatter
    }

    public func text(isInverse: Bool) -> String {
        let rate = isInverse ? rate.inverse : rate.direct
        return "\(formatter.string(1, symbol: rate.baseSymbol)) ≈ \(formatter.string(rate.value, symbol: rate.quoteSymbol))"
    }
}
