// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import struct BsvHomeCore.BsvHomeAmountEntry
import struct BsvHomeCore.BsvHomeAmountInput
import enum BsvHomeCore.BsvHomeAmountInputType
import enum BsvHomeCore.BsvHomeAmountType
import struct BsvHomeCore.BsvHomeAssetBalance
import struct BsvHomeCore.BsvHomeTransferData
import BsvHomeCorePrimitives
import Primitives

protocol AmountDataProvidable {
    var asset: Asset { get }
    var title: String { get }
    var bsvHomeAmountType: BsvHomeAmountType { get }
    var prefilledAmount: String? { get }
    func makeTransferData(value: BigInt, useMaxAmount: Bool) async throws -> BsvHomeTransferData
}

extension AmountDataProvidable {
    var prefilledAmount: String? {
        nil
    }

    func input(from assetData: AssetData) -> BsvHomeAmountInput {
        bsvHomeAmountType.input(asset: asset.map(), balance: BsvHomeAssetBalance(assetData.balance, assetId: asset.id, isActive: assetData.metadata.isActive))
    }

    func entry(from assetData: AssetData, inputType: BsvHomeAmountInputType, text: String) -> BsvHomeAmountEntry {
        bsvHomeAmountType.entry(
            asset: asset.map(),
            input: input(from: assetData),
            price: assetData.price?.price,
            inputType: inputType,
            text: text,
        )
    }
}
