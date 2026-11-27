// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeBannerAction
import struct BsvHomeCore.BsvHomeBannerContext
import struct BsvHomeCore.BsvHomeBannerKey
import Primitives

public extension Primitives.Banner {
    var bsvHomeKey: BsvHomeBannerKey {
        BsvHomeBannerKey(
            walletId: walletId?.id,
            assetId: asset?.id.identifier,
            event: event.map(),
        )
    }
}

public extension Primitives.BannerActionType {
    var bsvHomeAction: BsvHomeBannerAction {
        switch self {
        case let .event(event): .event(event: event.map())
        case .button: .button
        case .closeBanner: .close
        }
    }
}
