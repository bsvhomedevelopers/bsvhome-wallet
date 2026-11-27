// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import typealias BsvHomeCore.BannerState
import struct BsvHomeCore.BsvHomeBannerKey
import protocol BsvHomeCore.BsvHomeBannerStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreBannerStore: BsvHomeBannerStore, @unchecked Sendable {
    private let store: BannerStore

    public init(store: BannerStore) {
        self.store = store
    }

    public func getState(key: BsvHomeBannerKey) async throws -> BsvHomeCore.BannerState? {
        try store.getBanner(id: key.identifier())
            .map { $0.state.map() }
    }

    public func setState(key: BsvHomeBannerKey, state: BsvHomeCore.BannerState) async throws {
        let state = state.map()
        try store.addBanners([newBanner(key: key, state: state)])
        try store.updateState(key.identifier(), state: state)
    }

    public func addBanners(keys: [BsvHomeBannerKey], state: BsvHomeCore.BannerState) async throws {
        let state = state.map()
        try store.addBanners(keys.map { try newBanner(key: $0, state: state) })
    }

    private func newBanner(key: BsvHomeBannerKey, state: Primitives.BannerState) throws -> NewBanner {
        try NewBanner(
            id: key.identifier(),
            walletId: key.walletId,
            assetId: key.assetId.map { try Primitives.AssetId(id: $0) },
            event: key.event.map(),
            state: state,
        )
    }
}
