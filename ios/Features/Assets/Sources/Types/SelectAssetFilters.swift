// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeAssetFilter
import struct BsvHomeCore.BsvHomeSelectAssetFlow
import Store

extension BsvHomeSelectAssetFlow {
    var requestScope: AssetsRequestScope {
        switch scope {
        case .wallet: .wallet
        case .allAssets: .allAssets
        }
    }

    var requestFilters: [AssetsRequestFilter] {
        filters.map(AssetsRequestFilter.init(core:))
    }
}

extension AssetsRequestFilter {
    init(core filter: BsvHomeAssetFilter) {
        self = switch filter {
        case .enabled: .enabled
        case .buyable: .buyable
        case .sellable: .sellable
        case .swappable: .swappable
        case .hasBalance: .hasBalance
        case .hasAvailableBalance: .hasAvailableBalance
        case let .chainsOrAssetIds(chains, assetIds): .chainsOrAssets(chains, assetIds)
        }
    }
}
