// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeRecipient
import enum BsvHomeCore.TransactionInputType
import BigInt
import Foundation
import BsvHomeCorePrimitives
import struct BsvHomeCore.TransferDataExtra
import Primitives
import PrimitivesTestKit
import struct BsvHomeCore.BsvHomeTransferData

public extension BsvHomeTransferData {
    static func mock(
        type: TransactionInputType = .transfer(.mock()),
        recipient: BsvHomeRecipient = .mock(),
        value: BigInt = .zero,
        useMaxAmount: Bool = false,
    ) -> BsvHomeTransferData {
        BsvHomeTransferData(
            inputType: type,
            recipient: recipient,
            value: value,
            useMaxAmount: useMaxAmount,
        )
    }

    static func mockPayment(
        asset: Asset = .mockSolana(),
        transaction: String = "transaction",
        recipient: BsvHomeRecipient = .mock(),
        value: BigInt = .zero,
    ) -> BsvHomeTransferData {
        .mock(
            type: .generic(
                asset: asset,
                metadata: .mock(source: .payment),
                extra: .mock(data: Data(transaction.utf8)),
            ),
            recipient: recipient,
            value: value,
        )
    }
}
