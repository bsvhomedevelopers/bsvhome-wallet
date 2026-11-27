// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeConfirmMetadata
import BsvHomeCoreServices
import BigInt
import Foundation
import BsvHomeCorePrimitives
import struct BsvHomeCore.BsvHomeTransactionLoadFee
import Primitives
import PrimitivesComponents
import Style
import struct BsvHomeCore.BsvHomeTransferData
import class BsvHomeCore.BsvHomePerpetual

public struct TransactionInputViewModel: Sendable {
    let data: BsvHomeTransferData
    let fee: BsvHomeTransactionLoadFee?
    let metaData: BsvHomeConfirmMetadata?
    let transferAmount: TransferAmountValidation?
    let feeAsset: Asset

    private let currency: String

    public init(
        data: BsvHomeTransferData,
        fee: BsvHomeTransactionLoadFee?,
        metaData: BsvHomeConfirmMetadata?,
        transferAmount: TransferAmountValidation?,
        feeAsset: Asset,
        currency: String,
    ) {
        self.fee = fee
        self.data = data
        self.metaData = metaData
        self.transferAmount = transferAmount
        self.feeAsset = feeAsset
        self.currency = currency
    }

    var value: BigInt {
        switch transferAmount {
        case let .success(amount): amount.value
        case .failure, .none: data.value
        }
    }

    var asset: Asset {
        switch data.inputType {
        case let .perpetual(_, type): type.baseAsset
        default: data.asset
        }
    }

    var infoModel: TransactionInfoViewModel {
        TransactionInfoViewModel(
            currency: currency,
            asset: displayAsset,
            assetPrice: metaData?.assetPrice,
            feeAsset: feeAsset,
            feeAssetPrice: metaData?.feePrice,
            value: value,
            feeValue: fee?.fee,
            sign: .none,
        )
    }

    private var displayAsset: Asset {
        switch data.inputType {
        case .withdrawal: BsvHomePerpetual(provider: .hypercore).depositAsset().map()
        default: data.asset
        }
    }

    var networkFeeText: String? {
        infoModel.feeDisplay?.amount.text ?? Placeholder.empty
    }

    var networkFeeFiatText: String? {
        infoModel.feeDisplay?.fiat?.text
    }

    var headerType: TransactionHeaderType {
        TransactionHeaderTypeBuilder.build(
            infoModel: infoModel,
            transfer: data,
            metadata: metaData,
        )
    }

    var isReady: Bool {
        if case .success = transferAmount {
            return true
        }
        return false
    }
}
