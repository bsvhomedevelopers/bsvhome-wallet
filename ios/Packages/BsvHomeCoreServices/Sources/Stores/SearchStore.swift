// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import typealias BsvHomeCore.AssetId
import typealias BsvHomeCore.AssetList
import protocol BsvHomeCore.BsvHomeSearchStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreSearchStore: BsvHomeSearchStore, @unchecked Sendable {
    private let store: SearchStore
    private let assetListStore: AssetListStore

    public init(store: SearchStore, assetListStore: AssetListStore) {
        self.store = store
        self.assetListStore = assetListStore
    }

    public func setAssets(key: String, assetIds: [BsvHomeCore.AssetId]) async throws {
        try store.add(type: .asset, query: key, ids: assetIds)
    }

    public func setPerpetuals(key: String, perpetualIds: [String]) async throws {
        try store.add(type: .perpetual, query: key, ids: perpetualIds)
    }

    public func setLists(key: String, lists: [BsvHomeCore.AssetList]) async throws {
        let lists = lists.map { $0.map() }
        try assetListStore.upsert(lists)
        try store.add(type: .list, query: key, ids: lists.map(\.id))
    }
}
