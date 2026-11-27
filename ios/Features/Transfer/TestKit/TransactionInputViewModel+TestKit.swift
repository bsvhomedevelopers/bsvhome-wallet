// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeConfirmMetadata
import Foundation
import BsvHomeCorePrimitivesTestKit
import struct BsvHomeCore.BsvHomeTransactionLoadFee
import Primitives
import PrimitivesComponents
import PrimitivesTestKit
import Transfer
import struct BsvHomeCore.BsvHomeTransferData

public extension TransactionInputViewModel {
    static func mock(
        data: BsvHomeTransferData = .mock(),
        fee: BsvHomeTransactionLoadFee? = nil,
        metaData: BsvHomeConfirmMetadata? = nil,
        transferAmount: TransferAmountValidation? = nil,
        feeAsset: Asset = .mock(),
        currency: String = Currency.usd.rawValue,
    ) -> TransactionInputViewModel {
        TransactionInputViewModel(
            data: data,
            fee: fee,
            metaData: metaData,
            transferAmount: transferAmount,
            feeAsset: feeAsset,
            currency: currency,
        )
    }
}
