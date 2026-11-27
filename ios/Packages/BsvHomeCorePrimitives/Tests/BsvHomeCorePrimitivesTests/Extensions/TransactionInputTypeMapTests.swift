// Copyright (c). Gem Wallet. All rights reserved.

import BsvHomeCore
@testable import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import Primitives
import PrimitivesTestKit
import Testing

final class TransactionInputTypeMapTests {
    @Test
    func swapConstructorPreservesGasLimit() {
        let swapData = SwapData.mock(data: SwapQuoteData(
            to: "0x0000000000000000000000000000000000000001",
            dataType: .contract,
            value: 0,
            data: "0x",
            memo: nil,
            approval: .mock(),
            gasLimit: "500000",
        ))

        let mapped = TransactionInputType.swap(.mockEthereum(), .mockEthereum(), swapData)

        guard case let .swap(_, _, mappedSwapData) = mapped else {
            Issue.record("Expected swap input type")
            return
        }
        #expect(mappedSwapData.data.gasLimit == "500000")
    }
}
