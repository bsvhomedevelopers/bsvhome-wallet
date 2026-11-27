// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeSwapPairSuggestion
import Primitives

extension BsvHomeCore.BsvHomeSwapPairSuggestion {
    func map() throws -> SwapPairSelectorViewModel {
        try SwapPairSelectorViewModel(
            fromAssetId: AssetId(id: payAssetId),
            toAssetId: receiveAssetId.map { try AssetId(id: $0) },
        )
    }
}
