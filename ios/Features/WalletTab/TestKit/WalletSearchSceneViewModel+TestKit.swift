// Copyright (c). Gem Wallet. All rights reserved.

import protocol BsvHomeCore.BsvHomeAssetSelectionServiceProtocol
import class BsvHomeCore.BsvHomeRecentActivityService
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesTestKit
import Recents
import StoreTestKit
import WalletTab

public extension WalletSearchSceneViewModel {
    @MainActor
    static func mock(
        wallet: Wallet = .mock(),
        service: any BsvHomeAssetSelectionServiceProtocol = BsvHomeAssetSelectionServiceMock(),
    ) -> WalletSearchSceneViewModel {
        WalletSearchSceneViewModel(
            wallet: wallet,
            service: service,
            recentModel: RecentAssetsModel(
                walletId: wallet.id,
                types: RecentActivityType.allCases,
                allowedChains: service.filterChains(wallet: wallet.map()).map { Chain(core: $0) },
                service: BsvHomeRecentActivityService(store: BsvHomeCoreRecentActivityStore(store: .mock()), session: .mock()),
            ),
            onDismissSearch: {},
            onSelectAssetAction: { _ in },
            onAddToken: {},
        )
    }
}
