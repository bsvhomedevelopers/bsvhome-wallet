// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomePortfolioStore
import struct BsvHomeCore.PortfolioAsset
import typealias BsvHomeCore.WalletId
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCorePortfolioStore: BsvHomePortfolioStore, @unchecked Sendable {
    private let assetStore: AssetStore

    public init(assetStore: AssetStore) {
        self.assetStore = assetStore
    }

    public func getWalletAssets(walletId: BsvHomeCore.WalletId) async throws -> [BsvHomeCore.PortfolioAsset] {
        try assetStore.getAssetsData(walletId: Primitives.WalletId.from(id: walletId), filters: [.enabledBalance, .hasBalance], limit: nil)
            .map { BsvHomeCore.PortfolioAsset(assetId: $0.asset.id.identifier, value: $0.balance.total.magnitude) }
    }
}
