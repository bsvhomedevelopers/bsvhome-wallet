// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.SwapProvider
import struct BsvHomeCore.SwapperProviderData
import struct BsvHomeCore.SwapperRoute

extension SwapperProviderData {
    static func mock(provider: SwapProvider = .pancakeswapV3) -> SwapperProviderData {
        SwapperProviderData(
            provider: .mock(id: provider),
            slippageBps: 50,
            routes: [.mock()],
        )
    }
}
