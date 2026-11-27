import Components
import Formatters
import Foundation
import typealias BsvHomeCore.BsvHomeBigInt
import enum BsvHomeCore.BsvHomePortalSwapError
import struct BsvHomeCore.BsvHomePortalSwapPaymentPreparation
import protocol BsvHomeCore.BsvHomePortalSwapServiceProtocol
import protocol BsvHomeCore.BsvHomeTransactionCheckpoint
import struct BsvHomeCore.BsvHomeTransactionCheckpointData
import struct BsvHomeCore.BsvHomeTransferData
import BsvHomeCorePrimitives
import Localization
import Primitives

@MainActor
public final class PortalSwapPaymentContext: Identifiable {
    public let id: String
    public let wallet: Wallet
    public let transfer: BsvHomeTransferData
    public let checkpoint: any BsvHomeTransactionCheckpoint
    public let savedPayment: BsvHomeTransactionCheckpointData?
    public let service: any BsvHomePortalSwapServiceProtocol
    public let summaryItems: [ListItemModel]
    private var completion: ((Result<Void, any Error>) -> Void)?

    init(
        wallet: Wallet,
        preparation: BsvHomePortalSwapPaymentPreparation,
        savedPayment: BsvHomeTransactionCheckpointData?,
        service: any BsvHomePortalSwapServiceProtocol,
        completion: @escaping (Result<Void, any Error>) -> Void,
    ) throws {
        let payment = preparation.payment
        let order = preparation.order
        let transfer = preparation.transfer
        guard payment.walletId == wallet.id.identifier,
              payment.orderId == order.orderId,
              payment.payoutAddress == order.payoutAddress,
              payment.refundAddress == order.refundAddress,
              payment.depositAddress == transfer.recipient.address,
              payment.fromAmount == order.fromAmount,
              payment.estimatedToAmount == order.estimatedToAmount,
              let destination = order.to,
              destination.assetId == payment.payoutAssetId,
              transfer.asset.id.identifier == payment.fromAssetId,
              transfer.value == BsvHomeBigInt(payment.fromAmount)
        else {
            throw BsvHomePortalSwapError.OrderMismatch
        }
        id = payment.orderId
        self.wallet = wallet
        self.transfer = transfer
        self.checkpoint = preparation.checkpoint
        self.savedPayment = savedPayment
        self.service = service
        self.completion = completion
        let estimated = ValueFormatter(style: .full).string(
            BsvHomeBigInt(payment.estimatedToAmount),
            decimals: Int(destination.scale),
            currency: destination.symbol,
        )
        summaryItems = [
            ListItemModel(title: Localized.wowpaySwapOrderId, subtitle: payment.orderId, subtitleLineLimit: nil),
            ListItemModel(title: Localized.Common.provider, subtitle: payment.providerDisplayName),
            ListItemModel(title: Localized.wowpaySwapEstimatedReceive, subtitle: estimated),
            ListItemModel(title: Localized.Transfer.network, subtitle: destination.network),
            ListItemModel(title: Localized.Transaction.recipient, subtitle: payment.payoutAddress, subtitleLineLimit: nil),
            ListItemModel(title: Localized.wowpaySwapRefundAddress, subtitle: payment.refundAddress, subtitleLineLimit: nil),
            ListItemModel(title: Localized.wowpaySwapFloatingRateDescription, titleLineLimit: nil),
        ]
    }

    public func finish(_ result: Result<Void, any Error>) {
        guard let completion else { return }
        self.completion = nil
        if case let .failure(error) = result, error.isCancelled {
            service.cancel()
        }
        completion(result)
    }
}
