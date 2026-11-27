// Copyright (c). Gem Wallet. All rights reserved.

import BsvHomeCorePrimitivesTestKit
import Foundation
import Primitives
import PrimitivesTestKit
import Stake
import Testing

struct DelegationViewModelTests {
    @Test
    func balance() {
        let model = DelegationViewModel.mock()

        #expect(model.balanceText == "1,500.0000 TRX")
        #expect(model.fiatValueText == "$3,000.0000")
    }

    @Test
    func rewards() {
        let shown = DelegationViewModel.mock(rewardsShown: true)

        #expect(shown.rewardsText == "500.0000 TRX")
        #expect(shown.rewardsFiatValueText == "$1,000.0000")

        let hidden = DelegationViewModel.mock(rewardsShown: false)

        #expect(hidden.rewardsText == nil)
        #expect(hidden.rewardsFiatValueText == nil)
    }

    @Test
    func fractionalFiatValues() {
        let model = DelegationViewModel(
            service: BsvHomeStakeServiceMock(rewardsShown: true),
            delegation: .mock(
                price: .mock(price: 0.2),
                base: .mock(state: .active, assetId: .mock(.tron), balance: 100_000, rewards: 100_000),
            ),
            asset: Chain.tron.asset,
            currencyCode: "USD",
        )

        #expect(model.fiatValueText == "$0.0200")
        #expect(model.rewardsFiatValueText == "$0.0200")
    }

    @Test
    func completionDate() {
        let completionDate = Date.now.addingTimeInterval(86400)

        #expect(DelegationViewModel.mock(state: .pending, completionDate: completionDate).completionDateText == "23 hours, 59 minutes")
        #expect(DelegationViewModel.mock(state: .active, completionDate: completionDate).completionDateText == nil)
    }
}

extension DelegationViewModel {
    static func mock(
        state: DelegationState = .active,
        completionDate: Date? = nil,
        rewardsShown: Bool = false,
    ) -> DelegationViewModel {
        DelegationViewModel(
            service: BsvHomeStakeServiceMock(rewardsShown: rewardsShown),
            delegation: .mock(
                state: state,
                price: Price.mock(price: 2.0),
                base: .mock(
                    state: state,
                    assetId: .mock(.tron),
                    balance: 1_500_000_000,
                    rewards: 500_000_000,
                    completionDate: completionDate,
                ),
            ),
            asset: Chain.tron.asset,
            currencyCode: "USD",
        )
    }
}
