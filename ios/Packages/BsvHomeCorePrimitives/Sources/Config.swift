// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import typealias BsvHomeCore.ChainConfig
import class BsvHomeCore.Config
import enum BsvHomeCore.DocsUrl
import class BsvHomeCore.BsvHomeAddressService
import class BsvHomeCore.BsvHomeApplicationMetadataService
import class BsvHomeCore.BsvHomeAssetConfigService
import class BsvHomeCore.BsvHomeChainService
import class BsvHomeCore.BsvHomeConnectionService
import enum BsvHomeCore.NodeRegion
import class BsvHomeCore.PriceAlertFormatter
import enum BsvHomeCore.PublicUrl
import enum BsvHomeCore.RewardsUrl
import struct BsvHomeCore.SwapConfig
import typealias BsvHomeCore.WalletConnectConfig
import Primitives

public extension BsvHomeAddressService {
    static let shared = BsvHomeAddressService()
}

public extension BsvHomeApplicationMetadataService {
    static let shared = BsvHomeApplicationMetadataService()
}

public extension BsvHomeAssetConfigService {
    static let shared = BsvHomeAssetConfigService()
}

public extension BsvHomeChainService {
    static let shared = BsvHomeChainService.newWithEnvironment(environment: .current)
}

public extension BsvHomeConnectionService {
    static let shared = BsvHomeConnectionService()
}

public extension PriceAlertFormatter {
    static let shared = PriceAlertFormatter()
}

public extension Config {
    static let shared = Config()

    func swapConfig() -> SwapConfig {
        getSwapConfig()
    }
}

public enum NodeURL {
    public static let regions = Config.shared.getNodeRegions()

    public static func url(chain: Primitives.Chain, region: NodeRegion) -> URL {
        URL(string: Config.shared.getNodeUrl(chain: chain.rawValue, region: region))!
    }

    public static func region(url: String) -> NodeRegion? {
        Config.shared.getNodeRegion(url: url)
    }

    public static func flag(region: NodeRegion) -> String {
        Config.shared.getNodeRegionFlag(region: region)
    }

    public static func priority(region: NodeRegion) -> Int32 {
        Config.shared.getNodeRegionPriority(region: region)
    }
}

public enum AppUrl {
    private static let utmSource = "bsvhome_ios"

    public static func docs(_ item: DocsUrl) -> URL {
        URL(string: item.url())!
    }

    public static func page(_ item: PublicUrl) -> URL {
        let url = URL(string: item.url())!
        return switch item {
        case .website, .codebaseAndroid, .playStore, .apk:
            url.withUTM(source: utmSource)
        case .assets, .privacyPolicy, .termsOfService, .support, .codebaseIos, .appStore:
            url
        }
    }

    public static func rewards(_ item: RewardsUrl) -> URL {
        URL(string: item.url(locale: nil))!
    }
}

public enum ChainConfig {
    /// store in memory for fast access
    private static let chainConfigs: [Primitives.Chain: BsvHomeCore.ChainConfig] = Primitives.Chain.allCases.reduce(into: [:]) { result, chain in
        result[chain] = Config.shared.getChainConfig(chain: chain.rawValue)
    }

    public static func config(chain: Primitives.Chain) -> BsvHomeCore.ChainConfig {
        chainConfigs[chain]!
    }
}

public enum WalletConnectConfig {
    public static func config() -> BsvHomeCore.WalletConnectConfig {
        Config.shared.getWalletConnectConfig()
    }
}
