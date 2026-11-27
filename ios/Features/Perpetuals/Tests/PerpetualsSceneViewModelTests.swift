// Copyright (c). Gem Wallet. All rights reserved.

import protocol BsvHomeCore.BsvHomeRecentActivityServiceProtocol
import class BsvHomeCore.BsvHomeRecentActivityService
import Components
import BsvHomeCorePrimitivesTestKit
import protocol BsvHomeCore.BsvHomePerpetualServiceProtocol
import StoreTestKit
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
@testable import Perpetuals
import PerpetualsTestKit
import Primitives
import PrimitivesTestKit
import Testing

@MainActor
struct PerpetualsSceneViewModelTests {
    @Test
    func headerViewModel() {
        let wallet = Wallet.mock(type: .multicoin)
        let model = PerpetualsSceneViewModel.mock(wallet: wallet)

        #expect(model.headerViewModel.walletType == .multicoin)
    }

    @Test
    func pullToRefreshUpdatesMarketsThatTheTimerWouldSkip() async {
        let perpetuals = BsvHomePerpetualServiceMock()
        let model = PerpetualsSceneViewModel.mock(perpetualService: perpetuals)

        await model.load(source: .timer)
        #expect(perpetuals.syncMarketsCount == 1)

        await model.load(source: .timer)
        #expect(perpetuals.syncMarketsCount == 1)

        await model.load(source: .user)
        #expect(perpetuals.syncMarketsCount == 2)
    }

    @Test
    func openingTheSceneSyncsPositionsAndMarkets() async {
        let perpetuals = BsvHomePerpetualServiceMock()
        let model = PerpetualsSceneViewModel.mock(perpetualService: perpetuals)

        await model.load()

        #expect(perpetuals.syncPositionsCount == 1)
        #expect(perpetuals.syncMarketsCount == 1)
    }
}

extension PerpetualsSceneViewModel {
    @MainActor
    static func mock(
        wallet: Wallet = .mock(),
        perpetualService: any BsvHomePerpetualServiceProtocol = BsvHomePerpetualServiceMock(),
        observerService: any PerpetualObservable = PerpetualObserverMock(),
        recentAssetsService: any BsvHomeRecentActivityServiceProtocol = BsvHomeRecentActivityService(store: BsvHomeCoreRecentActivityStore(store: .mock()), session: .mock()),
    ) -> PerpetualsSceneViewModel {
        PerpetualsSceneViewModel(
            wallet: wallet,
            service: perpetualService,
            observerService: observerService,
            recentAssetsService: recentAssetsService,
        )
    }
}
