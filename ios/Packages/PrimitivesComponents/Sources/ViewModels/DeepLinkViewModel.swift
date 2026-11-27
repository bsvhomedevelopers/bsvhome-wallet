// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeSocialLink
import Primitives

struct DeepLinkViewModel {
    let link: BsvHomeSocialLink

    init(_ link: BsvHomeSocialLink) {
        self.link = link
    }

    var deepLink: URL? {
        guard let url = link.url.asURL, let host = url.host()?.lowercased() else { return nil }
        let path = url.path().trimmingPrefix("/")

        return switch link.linkType {
        case .telegram where ["t.me", "www.t.me", "telegram.me", "www.telegram.me"].contains(host):
            URL(string: "tg://resolve?domain=\(path)")
        case .x where ["x.com", "www.x.com", "twitter.com", "www.twitter.com"].contains(host):
            URL(string: "twitter://user?screen_name=\(path)")
        case .youTube where ["youtube.com", "www.youtube.com", "m.youtube.com", "youtu.be", "www.youtu.be"].contains(host):
            URL(string: "youtube://www.youtube.com/\(path)")
        case .discord where ["discord.gg", "www.discord.gg", "discord.com", "www.discord.com"].contains(host):
            URL(string: "https://discord.gg/\(path)")
        case .gitHub where ["github.com", "www.github.com"].contains(host):
            URL(string: "https://github.com/\(path)")
        case .telegram, .x, .youTube, .discord, .gitHub, .reddit, .facebook, .website, .coingecko, .openSea, .instagram, .magicEden, .coinMarketCap, .tikTok:
            nil
        }
    }
}
