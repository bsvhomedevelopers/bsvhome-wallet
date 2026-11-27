// Copyright (c). Gem Wallet. All rights reserved.

import BsvHomeCorePrimitives
import struct BsvHomeCore.BsvHomeConfirmMetadata
import protocol BsvHomeCore.BsvHomeConfirmSessionProtocol
import enum BsvHomeCore.TransactionInputType
import struct BsvHomeCore.BsvHomeSwapQuoteSummary
import func BsvHomeCore.perpetualDetails
import func BsvHomeCore.swapQuoteSummary
import BigInt
import Components
import Primitives
import PrimitivesComponents
import Swap

public struct ConfirmDetailsViewModel {
    private let type: TransactionInputType
    private let metadata: BsvHomeConfirmMetadata?
    private let session: any BsvHomeConfirmSessionProtocol

    init(
        type: TransactionInputType,
        metadata: BsvHomeConfirmMetadata?,
        session: any BsvHomeConfirmSessionProtocol,
    ) {
        self.type = type
        self.metadata = metadata
        self.session = session
    }
}

// MARK: - ItemModelProvidable

extension ConfirmDetailsViewModel: ItemModelProvidable {
    public var itemModel: ConfirmTransferItemModel {
        switch type {
        case let .swap(fromAsset, toAsset, swapData):
            let quote = swapData.quote
            let summary = swapQuoteSummary(quote: quote, fromAsset: fromAsset, toAsset: toAsset)
            let toAsset = toAsset.map()
            let fromAssetPrice = AssetPriceValue(asset: fromAsset.map(), price: metadata?.assetPrice)
            let toAssetPrice = AssetPriceValue(asset: toAsset, price: metadata?.assetPrices[toAsset.id])
            return .swapDetails(
                SwapDetailsViewModel(
                    fromAssetPrice: fromAssetPrice,
                    toAssetPrice: toAssetPrice,
                    selectedQuote: quote,
                    slippage: .manual(bps: quote.slippageBps),
                    rate: summary.rate,
                    currency: session.currency.rawValue,
                    swapPriceImpact: fromAssetPrice.swapValue(quote.fromValue)
                        .priceImpact(receive: toAssetPrice.swapValue(quote.toValue)),
                    minReceiveValue: BigInt(summary.minReceiveValue),
                    etaMinutes: summary.etaMinutes,
                ),
            )
        case let .perpetual(_, perpetualType):
            if case let .modify(data) = perpetualType {
                return .perpetualModifyPosition(PerpetualModifyViewModel(summary: session.autocloseSummary(data: data)))
            }
            guard let details = perpetualDetails(perpetualType: perpetualType) else { return .empty }
            return .perpetualDetails(PerpetualDetailsViewModel(details: details))
        case .transfer,
             .deposit,
             .withdrawal,
             .transferNft,
             .tokenApprove,
             .stake,
             .account,
             .generic,
             .earn:
            return .empty
        }
    }
}
