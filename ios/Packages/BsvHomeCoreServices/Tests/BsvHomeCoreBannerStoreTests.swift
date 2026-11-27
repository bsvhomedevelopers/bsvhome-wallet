// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeBannerKey
import BsvHomeCorePrimitives
@testable import BsvHomeCoreServices
import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit
import Testing

struct BsvHomeCoreBannerStoreTests {
    @Test
    func writesAndReadsBackTheStateCoreAsksFor() async throws {
        let db = DB.mockWithChains([.xrp, .cosmos])
        let store = BannerStore.mock(db: db)
        let adapter = BsvHomeCoreBannerStore(store: store)
        let wallet = Wallet.mock(id: .multicoin(address: "0xtest"), accounts: [.mock(chain: .xrp)])
        try WalletStore.mock(db: db).addWallet(wallet)
        let walletId = wallet.id
        let key = BsvHomeBannerKey(walletId: walletId.id, assetId: AssetId(chain: .xrp).identifier, event: Primitives.BannerEvent.accountActivation.map())

        try await adapter.addBanners(keys: [key], state: Primitives.BannerState.active.map())

        let row = try #require(try store.getBanner(id: key.identifier()))
        #expect(row.state == .active)
        #expect(row.walletId == walletId.id)
        #expect(row.assetId == AssetId(chain: .xrp))
        #expect(row.event == .accountActivation)
        #expect(try await adapter.getState(key: key) == Primitives.BannerState.active.map())
    }

    @Test
    func setStateCreatesTheRowWhenCoreHasNotSeededIt() async throws {
        let (store, adapter) = makeStore()
        let key = BsvHomeBannerKey(walletId: nil, assetId: AssetId(chain: .cosmos).identifier, event: Primitives.BannerEvent.stake.map())

        try await adapter.setState(key: key, state: Primitives.BannerState.cancelled.map())

        #expect(try store.getBanner(id: key.identifier())?.state == .cancelled)
    }

    @Test
    func addBannersLeavesAnExistingStateAlone() async throws {
        let (store, adapter) = makeStore()
        let key = BsvHomeBannerKey(walletId: nil, assetId: AssetId(chain: .cosmos).identifier, event: Primitives.BannerEvent.stake.map())
        try await adapter.setState(key: key, state: Primitives.BannerState.cancelled.map())

        try await adapter.addBanners(keys: [key], state: Primitives.BannerState.active.map())

        #expect(try store.getBanner(id: key.identifier())?.state == .cancelled)
    }

    private func makeStore() -> (BannerStore, BsvHomeCoreBannerStore) {
        let store = BannerStore.mock(db: .mockWithChains([.xrp, .cosmos]))
        return (store, BsvHomeCoreBannerStore(store: store))
    }
}
