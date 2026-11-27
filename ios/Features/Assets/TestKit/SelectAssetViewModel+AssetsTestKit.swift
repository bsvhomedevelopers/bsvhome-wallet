// Copyright (c). Gem Wallet. All rights reserved.

import protocol BsvHomeCore.BsvHomeAssetSelectionServiceProtocol
import class BsvHomeCore.BsvHomeRecentActivityService
import protocol BsvHomeCore.BsvHomeBalanceServiceProtocol
import BsvHomeCoreServicesTestKit
@testable import Assets
import BsvHomeCoreServices
import Components
import Foundation
import BsvHomeCorePrimitivesTestKit
import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit
@testable import Store

public extension SelectAssetViewModel {
    @MainActor
    static func mock(
        wallet: Wallet = .mock(),
        selectType: SelectAssetType = .manage,
        assets: [AssetData] = [],
        state: StateViewType<[AssetBasic]> = .noData,
        service: any BsvHomeAssetSelectionServiceProtocol = BsvHomeAssetSelectionServiceMock(),
    ) -> SelectAssetViewModel {
        let model = SelectAssetViewModel(
            wallet: wallet,
            selectType: selectType,
            service: service,
            recentAssetsService: BsvHomeRecentActivityService(store: BsvHomeCoreRecentActivityStore(store: .mock()), session: .mock()),
        )
        model.assetsQuery.value = assets
        model.state = state
        return model
    }
}
