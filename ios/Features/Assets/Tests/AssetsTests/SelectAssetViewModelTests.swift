// Copyright (c). Gem Wallet. All rights reserved.

@testable import Assets
import AssetsTestKit
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit
import Testing

@MainActor
struct SelectAssetViewModelTests {
    @Test(arguments: [WalletType.privateKey, .view])
    func persistedTestAssetsRemainOutsideWalletSelectionAndRecents(walletType: WalletType) throws {
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
        let model = SelectAssetViewModel.mock(wallet: wallet, selectType: .receive(.asset), service: service)
        model.assetsQuery.request.filters = []
        model.recentModel.query.request.filters = []
        let picker = model.recentModel.recentModel { _ in }

        let stored = try db.dbQueue.read { try AssetsRequest(walletId: wallet.id, allowedChains: chains, environment: .localDevelopment).fetch($0) }
        let selected = try db.dbQueue.read { try model.assetsQuery.request.fetch($0) }
        let recent = try db.dbQueue.read { try model.recentModel.query.request.fetch($0) }
        let pickerAssets = try db.dbQueue.read { try picker.query.request.fetch($0) }

        #expect(stored.count == 3)
        #expect(Set(stored.map(\.asset.id)) == Set(chains.map(\.assetId)))
        #expect(selected.map(\.asset.id) == [Chain.bitcoinSv.assetId])
        #expect(recent.map(\.asset.id) == [Chain.bitcoinSv.assetId])
        #expect(pickerAssets.map(\.asset.id) == [Chain.bitcoinSv.assetId])
        #expect(try WalletStore(db: db).getWallet(id: wallet.id)?.accounts.count == 3)
    }

    @Test
    func recentActivityTypes() {
        let model = SelectAssetViewModel.mock()

        #expect(model.recentModel.query.request.types == RecentActivityType.allCases)
    }

    @Test
    func showEmpty() {
        #expect(SelectAssetViewModel.mock(assets: []).showEmpty == true)
        #expect(SelectAssetViewModel.mock(assets: [AssetData.mock(metadata: .mock(isPinned: true))]).showEmpty == false)
        #expect(SelectAssetViewModel.mock(assets: [AssetData.mock(metadata: .mock(isPinned: false))]).showEmpty == false)
    }

    @Test
    func showLoading() {
        let pinnedAsset = AssetData.mock(metadata: .mock(isPinned: true))
        #expect(SelectAssetViewModel.mock(assets: [], state: .loading).showLoading == true)
        #expect(SelectAssetViewModel.mock(assets: [pinnedAsset], state: .loading).showLoading == false)
    }

    @Test
    func filterAndAddTokenRequireFlowAndWalletSupport() {
        let walletWithTokens = Wallet.mock(accounts: [.mock(chain: .ethereum)])
        let singleChainWallet = Wallet.mock(type: .single, accounts: [.mock(chain: .ethereum)])
        let withChains = BsvHomeAssetSelectionServiceMock()
        withChains.filterChainsResult = [Chain.ethereum.rawValue]
        let withoutTokens = BsvHomeAssetSelectionServiceMock()
        withoutTokens.filterChainsResult = [Chain.ethereum.rawValue]
        withoutTokens.tokensSupported = false

        #expect(SelectAssetViewModel.mock(wallet: walletWithTokens, selectType: .manage, service: withChains).showAddToken == true)
        #expect(SelectAssetViewModel.mock(wallet: walletWithTokens, selectType: .send(.none), service: withChains).showAddToken == false)
        #expect(SelectAssetViewModel.mock(wallet: walletWithTokens, selectType: .manage, service: withoutTokens).showAddToken == false)
        #expect(SelectAssetViewModel.mock(wallet: walletWithTokens, selectType: .manage).showAddToken == false)

        #expect(SelectAssetViewModel.mock(wallet: walletWithTokens, selectType: .manage, service: withChains).showFilter == true)
        #expect(SelectAssetViewModel.mock(wallet: walletWithTokens, selectType: .deposit, service: withChains).showFilter == false)
        #expect(SelectAssetViewModel.mock(wallet: singleChainWallet, selectType: .manage, service: withChains).showFilter == false)
    }

    @Test
    func toggleFlowEnablesAssets() async {
        await confirmation { enabledAssets in
            let enabler = BsvHomeAssetSelectionServiceMock(onSetAssetsEnabled: { assetIds, enabled in
                #expect(assetIds == [AssetId.mock().identifier])
                #expect(enabled == true)
                enabledAssets()
            })
            await SelectAssetViewModel.mock(selectType: .manage, service: enabler)
                .handleAction(assetId: .mock(), enabled: true)
        }
    }

    @Test
    func nonToggleFlowNeverEnablesAssets() async {
        await confirmation(expectedCount: 0) { enabledAssets in
            let enabler = BsvHomeAssetSelectionServiceMock(onSetAssetsEnabled: { _, _ in enabledAssets() })
            await SelectAssetViewModel.mock(selectType: .receive(.asset), service: enabler)
                .handleAction(assetId: .mock(), enabled: true)
        }
    }
}
