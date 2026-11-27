// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Components
import struct BsvHomeCore.BsvHomeSwapQuotesResult
import struct BsvHomeCore.BsvHomeSwapRequest
import struct BsvHomeCore.BsvHomeSwapSession
import enum BsvHomeCore.SwapperError
import Localization
import Primitives
import PrimitivesTestKit
import Style
@testable import Swap
import Testing

struct SwapButtonViewModelTests {
    @Test
    func retryTitleForBothRetryActions() {
        #expect(SwapButtonViewModel.mock(session: .mockFailed(.NoQuoteAvailable)).title == Localized.Common.tryAgain)
        #expect(SwapButtonViewModel.mock(session: .mockReady().failedTransfer(.NoQuoteAvailable)).title == Localized.Common.tryAgain)
    }

    @Test
    func retryQuotesStaysNormalWhileQuotesAreIdle() {
        let viewModel = SwapButtonViewModel.mock(session: .mockFailed(.NoQuoteAvailable))

        #expect(viewModel.type == ButtonType.primary(.normal))
        #expect(viewModel.isVisible == true)
    }

    @Test
    func retryTransferShowsLoadingWhileTheTransferIsInFlight() {
        let viewModel = SwapButtonViewModel.mock(session: BsvHomeSwapSession.mockReady().startTransfer()!)

        #expect(viewModel.type == ButtonType.primary(.loading()))
    }

    @Test
    func insufficientBalanceNamesTheAssetAndDisablesTheButton() {
        let asset = AssetData.mock(asset: .mock(symbol: "BTC"))
        let viewModel = SwapButtonViewModel.mock(session: .mockReady(), value: 2, availableBalance: 1, fromAsset: asset)

        #expect(viewModel.title == Localized.Transfer.insufficientBalance("BTC"))
        #expect(viewModel.type == ButtonType.primary(.disabled))
    }

    @Test
    func useMinimumAmountStaysEnabled() {
        let viewModel = SwapButtonViewModel.mock(session: .mockFailed(.InputAmountError(minAmount: "100")), availableBalance: 1000)

        #expect(viewModel.title == Localized.Swap.useMinimumAmount)
        #expect(viewModel.type == ButtonType.primary(.normal))
    }

    @Test
    func swapFollowsTheQuoteState() {
        #expect(SwapButtonViewModel.mock(session: .mockReady()).title == Localized.Wallet.swap)
        #expect(SwapButtonViewModel.mock(session: .mockReady()).type == ButtonType.primary(.normal))
        #expect(SwapButtonViewModel.mock(session: .mockLoading()).type == ButtonType.primary(.loading()))
        #expect(SwapButtonViewModel.mock(session: .mockFailed(.NoAvailableProvider)).type == ButtonType.primary(.disabled))
    }

    @Test
    func hiddenWhenNoQuotes() {
        #expect(SwapButtonViewModel.mock(session: .mock()).isVisible == false)
    }
}

extension SwapButtonViewModel {
    static func mock(
        session: BsvHomeSwapSession = .mock(),
        value: BigInt = 1,
        availableBalance: BigInt = 2,
        fromAsset: AssetData? = .mock(),
    ) -> SwapButtonViewModel {
        SwapButtonViewModel(
            state: session.viewState(value: value, availableBalance: availableBalance),
            fromAsset: fromAsset,
            onAction: {},
        )
    }
}

extension BsvHomeSwapRequest {
    static let mock = BsvHomeSwapRequest(
        payAssetId: AssetId.mockEthereum().identifier,
        receiveAssetId: AssetId.mockEthereumUSDT().identifier,
        value: 1_000_000_000_000_000_000,
        slippageBps: nil,
    )
}

extension BsvHomeSwapSession {
    static func mock() -> BsvHomeSwapSession {
        BsvHomeSwapSession(quotePhase: .noInput, transferPhase: .idle)
    }

    static func mockLoading() -> BsvHomeSwapSession {
        mock().onRequestChanged(request: .mock)
    }

    static func mockReady() -> BsvHomeSwapSession {
        mockLoading().onQuoteResults(results: BsvHomeSwapQuotesResult(request: .mock, quotes: [.mock()], error: nil))
    }

    static func mockFailed(_ error: SwapperError) -> BsvHomeSwapSession {
        mockLoading().onQuoteResults(results: BsvHomeSwapQuotesResult(request: .mock, quotes: [], error: error))
    }
}
