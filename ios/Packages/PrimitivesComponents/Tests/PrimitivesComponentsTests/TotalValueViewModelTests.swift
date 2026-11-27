// Copyright (c). Gem Wallet. All rights reserved.

@testable import PrimitivesComponents
import PrimitivesComponentsTestKit
import Style
import Testing

struct TotalValueViewModelTests {
    @Test
    func title() {
        #expect(TotalValueViewModel.mock(value: 1000).title == "$1,000.0000")
        #expect(TotalValueViewModel.mock(value: 0).title == "$0.0000")
    }

    @Test
    func pnlAmountText() {
        #expect(TotalValueViewModel.mock(pnlAmount: 50).pnlAmountText == "+$50.0000")
        #expect(TotalValueViewModel.mock(pnlAmount: -50).pnlAmountText == "-$50.0000")
        #expect(TotalValueViewModel.mock(pnlAmount: 0, showsPnl: false).pnlAmountText == nil)
    }

    @Test
    func pnlPercentageText() {
        #expect(TotalValueViewModel.mock(pnlPercentage: 5).pnlPercentageText == "5.0000%")
        #expect(TotalValueViewModel.mock(pnlPercentage: -5).pnlPercentageText == "5.0000%")
        #expect(TotalValueViewModel.mock(pnlAmount: 0, showsPnl: false).pnlPercentageText == nil)
    }

    @Test
    func pnlColor() {
        #expect(TotalValueViewModel.mock(pnlAmount: 50).pnlColor == Colors.green)
        #expect(TotalValueViewModel.mock(pnlAmount: -50).pnlColor == Colors.red)
        #expect(TotalValueViewModel.mock(pnlAmount: 0).pnlColor == Colors.gray)
    }
}
