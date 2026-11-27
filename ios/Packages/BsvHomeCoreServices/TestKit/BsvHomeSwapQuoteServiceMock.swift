// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import typealias BsvHomeCore.Asset
import typealias BsvHomeCore.AssetId
import typealias BsvHomeCore.Chain
import typealias BsvHomeCore.Currency
import enum BsvHomeCore.BsvHomeSlippageCheck
import struct BsvHomeCore.BsvHomeSwapQuoteSummary
import func BsvHomeCore.swapQuote
import protocol BsvHomeCore.BsvHomeSwapQuoteServiceProtocol
import struct BsvHomeCore.BsvHomeSwapPairSuggestion
import struct BsvHomeCore.BsvHomeSwapSession
import struct BsvHomeCore.BsvHomeSwapTransfer
import struct BsvHomeCore.SwapperQuote
import struct BsvHomeCore.SwapperSlippage
import struct BsvHomeCore.SwapQuoteData
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import Primitives
import PrimitivesTestKit

public final class BsvHomeSwapQuoteServiceMock: BsvHomeSwapQuoteServiceProtocol, @unchecked Sendable {
    private let quotes: @Sendable (BigInt) -> [SwapperQuote]
    private let quoteData: BsvHomeCore.SwapQuoteData
    private let quotesDelay: Duration?
    private let quotesError: Error?
    private let pairSuggestion: BsvHomeSwapPairSuggestion?
    private let slippageCheckResult: BsvHomeSlippageCheck
    public private(set) var storedSlippageBps: UInt32?

    public init(
        quotes: @escaping @Sendable (BigInt) -> [SwapperQuote],
        quoteData: BsvHomeCore.SwapQuoteData = .mock(),
        quotesDelay: Duration? = nil,
        quotesError: Error? = nil,
        pairSuggestion: BsvHomeSwapPairSuggestion? = nil,
        slippageBps: UInt32? = nil,
        slippageCheck: BsvHomeSlippageCheck = .valid,
    ) {
        self.quotes = quotes
        self.quoteData = quoteData
        self.quotesDelay = quotesDelay
        self.quotesError = quotesError
        self.pairSuggestion = pairSuggestion
        storedSlippageBps = slippageBps
        slippageCheckResult = slippageCheck
    }

    public convenience init(
        quotes: [SwapperQuote] = [.mock()],
        quoteData: BsvHomeCore.SwapQuoteData = .mock(),
        quotesDelay: Duration? = nil,
        quotesError: Error? = nil,
        pairSuggestion: BsvHomeSwapPairSuggestion? = nil,
        slippageBps: UInt32? = nil,
        slippageCheck: BsvHomeSlippageCheck = .valid,
    ) {
        self.init(
            quotes: { _ in quotes },
            quoteData: quoteData,
            quotesDelay: quotesDelay,
            quotesError: quotesError,
            pairSuggestion: pairSuggestion,
            slippageBps: slippageBps,
            slippageCheck: slippageCheck,
        )
    }

    public func getCurrency() -> Currency {
        Primitives.Currency.usd.rawValue
    }

    public func newSession() -> BsvHomeSwapSession {
        BsvHomeSwapSession(quotePhase: .noInput, transferPhase: .idle)
    }

    public func slippageBps() -> UInt32? {
        storedSlippageBps
    }

    public func setSlippageBps(bps: UInt32?) throws {
        storedSlippageBps = bps
    }

    public func slippageCheck(bps _: UInt32) -> BsvHomeSlippageCheck {
        slippageCheckResult
    }

    public func defaultSlippage(chain _: Chain) -> SwapperSlippage {
        SwapperSlippage(bps: 100, mode: .auto)
    }

    public func refreshIntervalMilliseconds() -> UInt64 {
        30_000
    }

    public func quoteDebounceMilliseconds() -> UInt64 {
        250
    }

    public func updateBalances(assetIds _: [AssetId]) async throws {}

    public func addPrices(assetIds _: [AssetId]) async throws {}

    public func getQuotes(fromAsset _: Asset, toAsset _: Asset, value: BigUInt, useMaxAmount _: Bool, slippageBps _: UInt32?) async throws -> [SwapperQuote] {
        if let quotesDelay {
            try await Task.sleep(for: quotesDelay)
        }
        if let quotesError {
            throw quotesError
        }
        return quotes(BigInt(value))
    }

    public func getTransfer(quote: SwapperQuote) async throws -> BsvHomeSwapTransfer {
        BsvHomeSwapTransfer(
            quote: BsvHomeCore.swapQuote(quote: quote),
            data: quoteData,
            recipient: quote.request.destinationAddress,
            value: quote.request.value,
            useMaxAmount: quote.request.options.useMaxAmount,
        )
    }

    public func suggestPair(payAssetId _: AssetId?) async throws -> BsvHomeSwapPairSuggestion? {
        pairSuggestion
    }
}
