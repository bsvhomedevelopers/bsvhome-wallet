// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeAssetConfigService
import BsvHomeCorePrimitives
import Primitives
import PrimitivesComponents
import Store

public extension TransactionsRequestFilter {
    static var activityDefaults: [TransactionsRequestFilter] {
        [.assetRankGreaterThan(BsvHomeAssetConfigService.shared.defaultTokenRank().asInt)]
    }
}
