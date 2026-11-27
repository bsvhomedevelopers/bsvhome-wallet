// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeSocialLink
import enum BsvHomeCore.LinkType
@testable import PrimitivesComponents
import Testing

struct DeepLinkViewModelTests {
    @Test(arguments: [
        (LinkType.telegram, "https://t.me/gemwallet", "tg://resolve?domain=gemwallet"),
        (.x, "https://x.com/GemWallet", "twitter://user?screen_name=GemWallet"),
        (.x, "https://twitter.com/GemWallet", "twitter://user?screen_name=GemWallet"),
        (.youTube, "https://www.youtube.com/@gemwallet", "youtube://www.youtube.com/@gemwallet"),
        (.discord, "https://discord.gg/aWkq5sj7SY", "https://discord.gg/aWkq5sj7SY"),
        (.gitHub, "https://github.com/gemwalletcom", "https://github.com/gemwalletcom"),
    ])
    func deepLinks(linkType: LinkType, url: String, expected: String) {
        let link = BsvHomeSocialLink(linkType: linkType, url: url, host: nil)
        #expect(DeepLinkViewModel(link).deepLink?.absoluteString == expected)
    }

    @Test(arguments: [
        (LinkType.telegram, "community"),
        (.x, "community"),
        (.youTube, "community"),
        (.discord, "community"),
        (.gitHub, "open-source"),
    ])
    func helpLinks(linkType: LinkType, path: String) {
        let url = "https://api-dev.hj0252.com/help/\(path)/"
        let link = BsvHomeSocialLink(linkType: linkType, url: url, host: nil)
        let insightLink = AssetLinkViewModel(link).insightLink

        #expect(insightLink?.deepLink == nil)
        #expect(insightLink?.url.absoluteString == url)
    }
}
