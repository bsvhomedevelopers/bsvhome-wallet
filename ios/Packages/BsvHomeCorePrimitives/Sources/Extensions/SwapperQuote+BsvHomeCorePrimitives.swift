// Copyright (c). Gem Wallet. All rights reserved.

import func BsvHomeCore.swapQuote
import struct BsvHomeCore.SwapperQuote
import struct BsvHomeCore.SwapQuote

public extension BsvHomeCore.SwapperQuote {
    var swapQuote: BsvHomeCore.SwapQuote {
        BsvHomeCore.swapQuote(quote: self)
    }
}
