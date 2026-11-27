// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import protocol BsvHomeCore.BsvHomeAmountServiceProtocol
import enum BsvHomeCore.BsvHomeAmountTransfer
import enum BsvHomeCore.BsvHomeAmountType
import struct BsvHomeCore.BsvHomePaymentRecipient
import struct BsvHomeCore.BsvHomeTransferData
import BsvHomeCorePrimitives
import Localization
import Primitives

public final class AmountTransferViewModel: AmountDataProvidable {
    let asset: Asset
    let transfer: BsvHomeAmountTransfer
    private let service: any BsvHomeAmountServiceProtocol

    init(asset: Asset, transfer: BsvHomeAmountTransfer, service: any BsvHomeAmountServiceProtocol) {
        self.asset = asset
        self.transfer = transfer
        self.service = service
    }

    var displayAsset: Asset {
        transfer.displayAsset(asset: asset.map()).map()
    }

    var title: String {
        switch transfer {
        case .send: Localized.Transfer.Send.title
        case .deposit: Localized.Wallet.deposit
        case .withdraw: Localized.Wallet.withdraw
        }
    }

    var bsvHomeAmountType: BsvHomeAmountType {
        transfer.amountType()
    }

    var prefilledAmount: String? {
        transfer.prefilledAmount()
    }

    func makeTransferData(value: BigInt, useMaxAmount: Bool) async throws -> BsvHomeTransferData {
        try await service.transferData(asset: asset.map(), transfer: transfer, value: value, useMaxAmount: useMaxAmount)
    }
}
