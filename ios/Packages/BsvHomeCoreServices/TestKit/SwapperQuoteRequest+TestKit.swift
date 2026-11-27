// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.SwapperOptions
import struct BsvHomeCore.SwapperQuoteAsset
import struct BsvHomeCore.SwapperQuoteRequest

extension SwapperQuoteRequest {
    static func mock() -> SwapperQuoteRequest {
        SwapperQuoteRequest(
            fromAsset: .mock(),
            toAsset: .mockUSDT(),
            walletAddress: "0x",
            destinationAddress: "0x",
            value: "1000000000000000000",
            options: .mock(),
        )
    }
}
