// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import Primitives
import PrimitivesTestKit
@testable import Swap
import Testing

struct PriceImpactViewModelTests {
    @Test
    func priceImpactValue_Low() {
        let model = PriceImpactViewModel.mock(fromValue: 1000000000, toValue: 990000000)
        let value = model.value

        #expect(value == PriceImpactValue(type: .low, value: "-1.0000%"))
    }

    @Test
    func priceImpactValue_Positive() {
        let model = PriceImpactViewModel.mock(fromValue: 1000000000, toValue: 1005000000)
        let value = model.value

        #expect(value == PriceImpactValue(type: .positive, value: "+0.5000%"))
    }

    @Test
    func priceImpactValue_Medium() {
        let model = PriceImpactViewModel.mock(fromValue: 1000000000, toValue: 950000000)
        let value = model.value

        #expect(value == PriceImpactValue(type: .medium, value: "-5.0000%"))
    }

    @Test
    func priceImpactValue_High() {
        let model = PriceImpactViewModel.mock(fromValue: 1000000000, toValue: 700000000)
        let value = model.value

        #expect(value == PriceImpactValue(type: .high, value: "-30.0000%"))
    }

    @Test
    func testShowPriceImpactWarning() {
        #expect(PriceImpactViewModel.mock(fromValue: 100, toValue: 109).showPriceImpactWarning == false)
        #expect(PriceImpactViewModel.mock(fromValue: 100, toValue: 111).showPriceImpactWarning == true)
        #expect(PriceImpactViewModel.mock(fromValue: 100, toValue: 120).showPriceImpactWarning == true)
    }

    @Test
    func testPriceImpactText() {
        #expect(PriceImpactViewModel.mock(fromValue: 100, toValue: 109).priceImpactText == "9.0000%")
        #expect(PriceImpactViewModel.mock(fromValue: 100, toValue: 111).priceImpactText == "11.0000%")
        #expect(PriceImpactViewModel.mock(fromValue: 100, toValue: 120).priceImpactText == "20.0000%")
    }
}

extension PriceImpactViewModel {
    static func mock(fromValue: BigUInt, toValue: BigUInt) -> PriceImpactViewModel {
        let assetPrice = AssetPriceValue(asset: .mockEthereum(), price: .mock())
        return PriceImpactViewModel(
            fromAssetPrice: assetPrice,
            swapPriceImpact: assetPrice.swapValue(fromValue)
                .priceImpact(receive: assetPrice.swapValue(toValue)),
        )
    }
}
