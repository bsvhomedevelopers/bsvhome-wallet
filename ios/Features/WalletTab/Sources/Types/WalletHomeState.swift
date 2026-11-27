// Copyright (c). Gem Wallet. All rights reserved.

import Primitives
import PrimitivesComponents

struct WalletHomeState {
    let header: WalletHeaderViewModel
    let balance: WalletBalanceViewModel
    let currencyCode: String
    let showPerpetuals: Bool
    let showCollections: Bool
    let visibleBanners: [Banner]
}
