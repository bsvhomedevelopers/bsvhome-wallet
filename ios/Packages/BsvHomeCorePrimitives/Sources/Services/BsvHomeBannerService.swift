// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeBannerContent
import protocol BsvHomeCore.BsvHomeBannerServiceProtocol
import Primitives

public extension BsvHomeBannerServiceProtocol {
    func content(for banner: Banner) -> BsvHomeBannerContent {
        bannerContent(event: banner.event.map(), asset: banner.asset?.map())
    }
}
