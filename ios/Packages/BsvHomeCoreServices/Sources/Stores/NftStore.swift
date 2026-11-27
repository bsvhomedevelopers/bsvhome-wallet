// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.NftAssetData
import typealias BsvHomeCore.NftAssetId
import struct BsvHomeCore.NftData
import protocol BsvHomeCore.BsvHomeNftStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreNftStore: BsvHomeNftStore, @unchecked Sendable {
    private let store: NftStore

    public init(store: NftStore) {
        self.store = store
    }

    public func saveNfts(walletId: String, data: [BsvHomeCore.NftData]) async throws {
        try store.save(data.map { $0.map() }, for: WalletId.from(id: walletId))
    }

    public func getAssetData(assetId: BsvHomeCore.NftAssetId) async throws -> BsvHomeCore.NftAssetData? {
        guard let asset = try store.getAsset(assetId: NFTAssetId.from(id: assetId)),
              let collection = try store.getCollection(collectionId: asset.collectionId)
        else {
            return nil
        }
        return NFTAssetData(collection: collection, asset: asset).map()
    }

    public func saveAsset(data: BsvHomeCore.NftAssetData) async throws {
        let data = data.map()
        try store.add(asset: data.asset, collection: data.collection)
    }
}
