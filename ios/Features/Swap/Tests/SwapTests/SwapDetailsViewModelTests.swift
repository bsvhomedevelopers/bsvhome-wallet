// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Components
import Formatters
import struct BsvHomeCore.BsvHomeSwapQuoteSummary
import func BsvHomeCore.swapQuoteSummary
import BsvHomeCorePrimitives
import struct BsvHomeCore.SwapperQuote
import struct BsvHomeCore.SwapQuote
import Primitives
import PrimitivesTestKit
@testable import Swap
import BsvHomeCoreServicesTestKit
import Testing

@MainActor
struct SwapDetailsViewModelTests {
    @Test
    func swapEstimationField() throws {
        #expect(
            try SwapDetailsViewModel
                .mock(selectedQuote: SwapperQuote.mock(etaInSeconds: nil).swapQuote).swapEstimationField == nil,
        )
        #expect(SwapDetailsViewModel.mock(selectedQuote: SwapperQuote.mock(etaInSeconds: 30).swapQuote).swapEstimationField == nil)
        #expect(SwapDetailsViewModel.mock(selectedQuote: SwapperQuote.mock(etaInSeconds: 180).swapQuote).swapEstimationField?.value.text == "≈ 3 min")
    }

    @Test
    func switchRate() throws {
        let model = SwapDetailsViewModel.mock(selectedQuote: SwapperQuote.mock(toValue: 250_000_000_000).swapQuote)

        #expect(model.rateText == "1.0000 ETH ≈ 250,000.0000 USDT")

        model.switchRateDirection()
        #expect(model.rateText == "1.0000 USDT ≈ 0.000004 ETH")
    }

    @Test
    func minReceiveAppliesSlippageBasisPoints() throws {
        let model = SwapDetailsViewModel.mock(selectedQuote: SwapperQuote.mock(toValue: 250_000_000_000).swapQuote)

        #expect(model.minReceiveField.value.text == "248,750.0000 USDT")
    }
}

extension SwapDetailsViewModel {
    static func mock(selectedQuote: BsvHomeCore.SwapQuote = SwapperQuote.mock().swapQuote) -> SwapDetailsViewModel {
        let summary = swapQuoteSummary(quote: selectedQuote, fromAsset: Asset.mockEthereum().map(), toAsset: Asset.mockEthereumUSDT().map())
        return SwapDetailsViewModel(
            fromAssetPrice: AssetPriceValue(asset: .mockEthereum(), price: .mock()),
            toAssetPrice: AssetPriceValue(asset: .mockEthereumUSDT(), price: .mock()),
            selectedQuote: selectedQuote,
            slippage: .auto,
            rate: summary.rate,
            currency: Currency.usd.rawValue,
            swapPriceImpact: nil,
            minReceiveValue: BigInt(summary.minReceiveValue),
            etaMinutes: summary.etaMinutes,
            swapProviderSelectAction: nil,
        )
    }
}
