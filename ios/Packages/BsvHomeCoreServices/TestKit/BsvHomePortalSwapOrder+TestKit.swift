import enum BsvHomeCore.BsvHomePortalSwapDirection
import struct BsvHomeCore.BsvHomePortalSwapOrder
import enum BsvHomeCore.BsvHomePortalSwapOrderStatus
import struct BsvHomeCore.BsvHomePortalSwapQuote
import enum BsvHomeCore.BsvHomePortalSwapResolutionStatus
import enum BsvHomeCore.BsvHomePortalSwapVerificationStatus

public extension BsvHomePortalSwapOrder {
    static func mock(
        orderId: String = "order",
        direction: BsvHomePortalSwapDirection? = .bsvToUsdt,
        orderStatus: BsvHomePortalSwapOrderStatus = .awaitingSignature,
        verificationStatus: BsvHomePortalSwapVerificationStatus = .notRequired,
        resolutionStatus: BsvHomePortalSwapResolutionStatus = .open,
        canPay: Bool = true,
    ) -> BsvHomePortalSwapOrder {
        let quote = direction.map { BsvHomePortalSwapQuote.mock(direction: $0) }
        return BsvHomePortalSwapOrder(
            orderId: orderId,
            clientOperationId: "operation",
            direction: direction,
            from: quote?.from,
            to: quote?.to,
            fromAmount: 100_000_000,
            fromAmountDisplay: quote?.fromAmountDisplay,
            estimatedToAmount: 100_000_000,
            estimatedToAmountDisplay: quote?.estimatedToAmountDisplay,
            actualToAmountDisplay: nil,
            actualToAmount: nil,
            payoutVerified: false,
            depositAddress: "deposit",
            depositExtraId: nil,
            payoutAddress: "payout",
            refundAddress: "refund",
            providerDisplayName: "Provider",
            providerOrderId: nil,
            orderStatus: orderStatus,
            verificationStatus: verificationStatus,
            syncStatus: .fresh,
            resolutionStatus: resolutionStatus,
            lastActivePhase: nil,
            sourceTxid: nil,
            broadcastReportPending: false,
            payoutTxid: nil,
            refundTxid: nil,
            depositExpiresAt: nil,
            updatedAt: 1_000_000,
            version: 1,
            reasonCode: nil,
            requiredActions: [],
            canPay: canPay,
        )
    }
}
