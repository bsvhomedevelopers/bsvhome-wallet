// Copyright (c). Gem Wallet. All rights reserved.
import Foundation
import struct BsvHomeCore.BsvHomeSocialLink

public struct SocialLinksViewModel {
    private let socialLinks: [BsvHomeSocialLink]

    public init(links: [BsvHomeSocialLink]) {
        socialLinks = links
    }

    var links: [InsightLink] {
        socialLinks.compactMap { AssetLinkViewModel($0).insightLink }
    }
}
