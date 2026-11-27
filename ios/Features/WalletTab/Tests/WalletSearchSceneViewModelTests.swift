// Copyright (c). Gem Wallet. All rights reserved.

import BsvHomeCorePrimitives
import BsvHomeCoreServicesTestKit
import NFT
import Primitives
import PrimitivesTestKit
@testable import Store
import StoreTestKit
import Testing
@testable import WalletTab
import WalletTabTestKit

@MainActor
struct WalletSearchSceneViewModelTests {
    @Test(arguments: [WalletType.privateKey, .view])
    func persistedTestAssetsRemainOutsideWalletSearchAndRecents(walletType: WalletType) throws {
        let chains: [Chain] = [.bitcoinSv, .tron, .tronShasta]
        let db = DB.mockWithChains(chains)
        let wallet = Wallet.mock(
            id: .make(walletType: walletType, chain: .bitcoinSv, address: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5"),
            type: walletType,
            accounts: chains.map { .mock(chain: $0, address: $0 == .bitcoinSv ? "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5" : "historical-address") },
        )
        try WalletStore(db: db).addWallet(wallet)
        try BalanceStore(db: db).addBalance(assetIds: chains.map(\.assetId), isEnabled: true, for: wallet.id)
        for chain in chains {
            try RecentActivityStore(db: db).add(assetId: chain.assetId, toAssetId: nil, walletId: wallet.id, type: .transfer)
        }
        let service = BsvHomeAssetSelectionServiceMock()
        service.filterChainsResult = [Chain.bitcoinSv.rawValue]
        let model = WalletSearchSceneViewModel.mock(wallet: wallet, service: service)
        let stored = try db.dbQueue.read { try WalletSearchRequest(walletId: wallet.id, allowedChains: chains, environment: .localDevelopment).fetch($0) }
        let search = try db.dbQueue.read { try model.searchQuery.request.fetch($0) }
        let recent = try db.dbQueue.read { try model.recentModel.query.request.fetch($0) }
        let picker = model.recentModel.recentModel { _ in }
        let pickerAssets = try db.dbQueue.read { try picker.query.request.fetch($0) }

        #expect(stored.assets.count == 3)
        #expect(Set(stored.assets.map(\.asset.id)) == Set(chains.map(\.assetId)))
        #expect(search.assets.map(\.asset.id) == [Chain.bitcoinSv.assetId])
        #expect(recent.map(\.asset.id) == [Chain.bitcoinSv.assetId])
        #expect(pickerAssets.map(\.asset.id) == [Chain.bitcoinSv.assetId])
        #expect(try WalletStore(db: db).getWallet(id: wallet.id)?.accounts.count == 3)
    }

    @Test
    func searchRequestInitialization() {
        #expect(WalletSearchSceneViewModel.mock().searchQuery.request.limit == 13)
        #expect(WalletSearchSceneViewModel.mock().searchQuery.request.types == [.asset, .perpetual, .list, .nft])
    }

    @Test
    func hasMoreAssets() {
        let model = WalletSearchSceneViewModel.mock()

        model.searchQuery.value = .mock(assets: (0 ..< 12).map { _ in .mock() })
        #expect(model.hasMoreAssets == false)

        model.searchQuery.value = .mock(assets: (0 ..< 13).map { _ in .mock() })
        #expect(model.hasMoreAssets == true)
    }

    @Test
    func hasMorePerpetuals() {
        let model = WalletSearchSceneViewModel.mock()

        model.searchQuery.value = .mock(perpetuals: (0 ..< 3).map { _ in .mock() })
        #expect(model.hasMorePerpetuals == false)

        model.searchQuery.value = .mock(perpetuals: (0 ..< 4).map { _ in .mock() })
        #expect(model.hasMorePerpetuals == true)
    }

    @Test
    func hidesPerpetualsWhenTheServiceSaysSo() {
        let service = BsvHomeAssetSelectionServiceMock()
        service.perpetualsShown = false
        let model = WalletSearchSceneViewModel.mock(service: service)
        model.searchQuery.value = .mock(
            perpetuals: [
                .mock(metadata: .mock(isPinned: false)),
                .mock(metadata: .mock(isPinned: true)),
            ],
        )
        #expect(model.showPerpetuals == false)
        #expect(model.showPinnedPerpetuals == false)
    }

    @Test
    func listsSection() {
        let model = WalletSearchSceneViewModel.mock()

        #expect(model.showLists == false)

        let list = AssetList(id: "stocks", name: "Stocks", count: 2)
        model.searchQuery.value = .mock(lists: [list])

        #expect(model.showLists == true)
        #expect(model.showEmpty == false)
        #expect(model.listDestination(for: list) == Scenes.AssetsResults(searchQuery: "", scope: .list("stocks"), title: "Stocks"))
    }

    @Test
    func hasMoreNFTs() {
        let service = BsvHomeAssetSelectionServiceMock()
        let model = WalletSearchSceneViewModel.mock(service: service)

        service.nftSearchItems = (0 ..< 3).map { _ in .asset(data: NFTAssetData.mock().map()) }
        #expect(model.hasMoreNFTs == false)

        service.nftSearchItems = (0 ..< 4).map { _ in .asset(data: NFTAssetData.mock().map()) }
        #expect(model.hasMoreNFTs == true)
    }

    @Test
    func nftsSection() {
        let service = BsvHomeAssetSelectionServiceMock()
        let model = WalletSearchSceneViewModel.mock(service: service)

        #expect(model.showNFTs == false)

        service.nftSearchItems = [
            .collection(data: NFTData(collection: .mock(), assets: [.mock(), .mock()]).map()),
            .asset(data: NFTAssetData.mock().map()),
        ]

        #expect(model.showNFTs == true)
        #expect(model.showEmpty == false)
        #expect(model.collectionsContent.items.count == 2)
    }

    @Test
    func pinAssetPinsThroughTheService() async {
        let pinned: (assetId: String, pinned: Bool) = await withCheckedContinuation { continuation in
            let model = WalletSearchSceneViewModel.mock(
                service: BsvHomeAssetSelectionServiceMock(onSetAssetPinned: { assetId, pinned in continuation.resume(returning: (assetId, pinned)) }),
            )
            model.onPinAsset(.mock(), value: true)
        }

        #expect(pinned.assetId == AssetId.mock().identifier)
        #expect(pinned.pinned)
    }
}
