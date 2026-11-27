// Copyright (c). Gem Wallet. All rights reserved.

import Primitives
import PrimitivesTestKit
@testable import Stake
import protocol BsvHomeCore.BsvHomeStakeServiceProtocol
import BsvHomeCorePrimitivesTestKit

public extension StakeSceneViewModel {
    static func mock(
        wallet: Wallet = .mock(),
        chain: StakeChain = .tron,
        stakeService: any BsvHomeStakeServiceProtocol = BsvHomeStakeServiceMock(),
    ) -> StakeSceneViewModel {
        StakeSceneViewModel(
            wallet: wallet,
            chain: chain,
            service: stakeService,
        )
    }
}
