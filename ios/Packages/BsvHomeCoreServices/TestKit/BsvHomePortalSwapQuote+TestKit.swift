import enum BsvHomeCore.BsvHomePortalSwapDirection
import struct BsvHomeCore.BsvHomePortalSwapPair
import struct BsvHomeCore.BsvHomePortalSwapQuote

public extension BsvHomePortalSwapQuote {
    static func mock(quoteId: String = "quote", direction: BsvHomePortalSwapDirection = .bsvToUsdt, expiresAt: Int64 = 4_000_000_000) -> BsvHomePortalSwapQuote {
        let pair = BsvHomePortalSwapPair.mock(direction: direction)
        let fromAmount = pair.from.scale == 8 ? "1" : "100"
        let toAmount = pair.to.scale == 8 ? "1" : "100"
        let minimum = pair.from.scale == 8 ? "0.1" : "10"
        let rate = switch (pair.from.scale, pair.to.scale) {
        case (8, 6): "100"
        case (6, 8): "0.01"
        default: "1"
        }
        return BsvHomePortalSwapQuote(
            quoteId: quoteId,
            direction: direction,
            from: pair.from,
            to: pair.to,
            fromAmount: 100_000_000,
            fromAmountDisplay: "\(fromAmount) \(pair.from.symbol)",
            estimatedToAmount: 100_000_000,
            estimatedToAmountDisplay: "\(toAmount) \(pair.to.symbol)",
            minFromAmountDisplay: "\(minimum) \(pair.from.symbol)",
            maxFromAmountDisplay: nil,
            rateDisplay: "1 \(pair.from.symbol) ≈ \(rate) \(pair.to.symbol)",
            providerDisplayName: "Provider",
            expiresAt: expiresAt,
        )
    }
}
