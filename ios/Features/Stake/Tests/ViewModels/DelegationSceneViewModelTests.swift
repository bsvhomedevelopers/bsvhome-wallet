// Copyright (c). Gem Wallet. All rights reserved.

import BsvHomeCorePrimitivesTestKit
import Foundation
import Primitives
import PrimitivesTestKit
@testable import Stake
import StakeTestKit
import Testing

struct DelegationSceneViewModelTests {
    @Test
    func rewardsShownWhenCoreReportsThem() {
        let claimable = DelegationSceneViewModel.mock(stakeService: BsvHomeStakeServiceMock(claimable: true))
        let notClaimable = DelegationSceneViewModel.mock(stakeService: BsvHomeStakeServiceMock(claimable: false))

        #expect(claimable.canClaimRewards == true)
        #expect(notClaimable.canClaimRewards == false)
    }
}
