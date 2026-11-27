// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeAssetAction
import Primitives

public extension SelectedAssetType {
    var action: BsvHomeAssetAction? {
        switch self {
        case .send: .send
        case .receive: .receive
        case .buy: .buy
        case .sell: .sell
        case .stake, .earn, .swap: .none
        }
    }
}
