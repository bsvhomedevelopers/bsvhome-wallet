// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import BsvHomeCore
import class BsvHomeCore.BsvHomeAssetConfigService
import Primitives

private let chainAssets: [Primitives.Chain: Primitives.ChainAsset] = Primitives.Chain.allCases.reduce(into: [:]) { result, chain in
    result[chain] = BsvHomeAssetConfigService.shared.chainAsset(chain: chain.rawValue).map()
}

public extension Primitives.Chain {
    static let availableCases: [Self] = BsvHomeChainService.newWithEnvironment(environment: .current).getChains(query: "").map { Self(core: $0) }
    static let productionCases: [Self] = BsvHomeChainService().getChains(query: "").map { Self(core: $0) }

    var asset: Primitives.Asset {
        chainAsset.asset
    }

    var networkName: String {
        chainAsset.networkName
    }

    var minimumAccountBalance: BigInt {
        BigInt(ChainConfig.config(chain: self).minimumAccountBalance ?? .zero)
    }

    var isMemoSupported: Bool {
        ChainConfig.config(chain: self).isMemoSupported
    }

    var type: Primitives.ChainType {
        ChainConfig.config(chain: self).chainType.map()
    }

    var iconChain: Primitives.Chain {
        Primitives.Chain(core: ChainConfig.config(chain: self).iconChain)
    }

    func defaultAsset(type: Primitives.AssetType) -> Primitives.Asset {
        guard let asset = BsvHomeAssetConfigService.shared.defaultAsset(chain: map(), assetType: type.map()) else {
            preconditionFailure("Missing \(type) default asset for \(self)")
        }
        return asset.map()
    }
}

private extension Primitives.Chain {
    var chainAsset: Primitives.ChainAsset {
        guard let asset = chainAssets[self] else {
            preconditionFailure("Missing chain asset for \(self)")
        }
        return asset
    }
}

public extension [Primitives.Asset] {
    func matching(query: String) -> [Primitives.Asset] {
        let assets = map { $0.map() }
        return BsvHomeAssetConfigService.shared.matchingAssets(assets: assets, query: query).map { $0.map() }
    }
}
