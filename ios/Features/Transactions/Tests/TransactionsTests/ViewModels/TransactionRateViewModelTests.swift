// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeAssetRate
import struct BsvHomeCore.BsvHomeSwapRate
import Testing
@testable import Transactions

struct TransactionRateViewModelTests {
    @Test
    func itemModel() {
        let rate = BsvHomeSwapRate(
            direct: BsvHomeAssetRate(baseSymbol: "ETH", quoteSymbol: "USDT", value: 3000),
            inverse: BsvHomeAssetRate(baseSymbol: "USDT", quoteSymbol: "ETH", value: 1 / 3000),
        )

        guard
            case let .rate(_, direct) = TransactionRateViewModel(rate: rate, isInverse: false).itemModel,
            case let .rate(_, inverse) = TransactionRateViewModel(rate: rate, isInverse: true).itemModel
        else {
            Issue.record("Expected rate item")
            return
        }
        #expect(direct.hasPrefix("1.0000 ETH"))
        #expect(inverse.hasPrefix("1.0000 USDT"))

        if case .empty = TransactionRateViewModel(rate: nil, isInverse: false).itemModel {
        } else {
            Issue.record("Expected empty without a rate")
        }
    }
}
