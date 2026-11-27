// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeRecentActivity
import Primitives

public extension RecentActivityData {
    init(_ activity: BsvHomeRecentActivity) {
        self.init(
            type: activity.activityType.map(),
            assetId: Primitives.AssetId(core: activity.assetId),
            toAssetId: activity.toAssetId.map { Primitives.AssetId(core: $0) },
        )
    }
}
