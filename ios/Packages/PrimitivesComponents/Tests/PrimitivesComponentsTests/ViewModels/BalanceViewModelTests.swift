// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import Primitives
@testable import PrimitivesComponents
import PrimitivesComponentsTestKit
import PrimitivesTestKit
import Testing

struct BalanceViewModelTests {
    @Test
    func total() {
        let tronBalance = Balance(
            available: BigInt(1),
            frozen: BigInt(1_000_000),
            locked: BigInt(0),
            staked: BigInt(0),
            pending: BigInt(0),
            rewards: BigInt(5496),
            metadata: BalanceMetadata(
                votes: 1,
                energyAvailable: .zero,
                energyTotal: .zero,
                bandwidthAvailable: .zero,
                bandwidthTotal: .zero,
            ),
        )
        let tronModel = BalanceViewModel(asset: .mockTron(), balance: tronBalance, formatter: .auto)

        let bnbBalance = Balance(
            available: BigInt(1_000_000),
            frozen: BigInt(0),
            locked: BigInt(0),
            staked: BigInt(5_000_000),
            pending: BigInt(0),
            rewards: BigInt(100_000),
        )
        let bnbModel = BalanceViewModel(asset: .mockBNB(), balance: bnbBalance, formatter: .auto)

        #expect(tronModel.total == BigInt(1_005_497))
        #expect(bnbModel.total == BigInt(6_100_000))
    }

    @Test
    func balanceTextWithSymbol() {
        let model = BalanceViewModel.mock(asset: .mockEthereum(), formatter: .full)
        let emptyModel = BalanceViewModel.mock(asset: .mockEthereum(), balance: .mock(available: .zero), formatter: .full)

        #expect(model.balanceTextWithSymbol(BigInt(1_000_000_000_000_000_000)) == "1.0000 ETH")
        #expect(model.balanceTextWithSymbol(.zero) == "0.0000 ETH")
        #expect(emptyModel.balanceText == "0.0000")
        #expect(emptyModel.availableBalanceText == "0.0000")
    }

    @Test
    func usdtBalancesShowExactlyFourDecimalsOnEveryNetwork() {
        for asset in [Asset.mockTronUSDT(), .mockEthereumUSDT()] {
            let model = BalanceViewModel.mock(asset: asset, balance: .mock(available: 123_456_789), formatter: .full)

            #expect(model.balanceText == "123.4567")
            #expect(model.availableBalanceText == "123.4567")
            #expect(model.totalBalanceTextWithSymbol == "123.4567 USDT")
            #expect(model.availableBalanceTextWithSymbol == "123.4567 USDT")
            #expect(model.balanceTextWithSymbol(1_234_567) == "1.2345 USDT")
            #expect(model.availableBalanceInputText == "123.456789")
        }
    }

    @Test
    func usdtBalancesKeepCompactStyleAndPadSmallValues() {
        let asset = Asset.mockTronUSDT()
        let large = BalanceViewModel.mock(asset: asset, balance: .mock(available: 123_456_789_123), formatter: .short)
        let small = BalanceViewModel.mock(asset: asset, balance: .mock(available: 1), formatter: .full)

        #expect(large.balanceText == "123.4567K")
        #expect(small.balanceText == "0.0000")
        #expect(small.availableBalanceInputText == "0.000001")
    }

}
