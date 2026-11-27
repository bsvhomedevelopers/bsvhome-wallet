// Copyright (c). Gem Wallet. All rights reserved.

import Components
import enum BsvHomeCore.BsvHomeHeaderButtonKind

public extension BsvHomeHeaderButtonKind {
    var headerButtonType: HeaderButtonType {
        switch self {
        case .send: .send
        case .receive: .receive
        case .buy: .buy
        case .swap: .swap
        }
    }
}
