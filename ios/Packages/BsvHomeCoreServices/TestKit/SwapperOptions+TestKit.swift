// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.SwapperOptions
import struct BsvHomeCore.SwapperSlippage
import enum BsvHomeCore.SwapperSlippageMode

extension SwapperOptions {
    static func mock() -> SwapperOptions {
        SwapperOptions(
            slippage: SwapperSlippage(
                bps: 50,
                mode: .auto,
            ),
            useMaxAmount: false,
        )
    }
}
