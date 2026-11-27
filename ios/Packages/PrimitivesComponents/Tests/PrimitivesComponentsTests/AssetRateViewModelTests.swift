// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import Foundation
import struct BsvHomeCore.BsvHomeAssetRate
import struct BsvHomeCore.BsvHomeSwapRate
@testable import PrimitivesComponents
import Testing

struct AssetRateViewModelTests {
    @Test
    func textShowsOneBaseUnitInEachDirection() {
        let model = AssetRateViewModel(
            rate: BsvHomeSwapRate(
                direct: BsvHomeAssetRate(baseSymbol: "ETH", quoteSymbol: "USDT", value: 250_000),
                inverse: BsvHomeAssetRate(baseSymbol: "USDT", quoteSymbol: "ETH", value: 0.000004),
            ),
            formatter: NumericFormatter(locale: Locale(identifier: "en_US")),
        )

        #expect(model.text(isInverse: false) == "1.0000 ETH ≈ 250,000.0000 USDT")
        #expect(model.text(isInverse: true) == "1.0000 USDT ≈ 0.000004 ETH")
    }
}
