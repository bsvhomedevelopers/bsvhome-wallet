// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import enum BsvHomeCore.BsvHomeAmountType
import protocol BsvHomeCore.BsvHomeAmountServiceProtocol
import BsvHomeCorePrimitives
import Localization
import Primitives
import struct BsvHomeCore.BsvHomeTransferData

public final class AmountEarnViewModel: AmountDataProvidable {
    let asset: Asset
    let action: EarnType
    private let service: any BsvHomeAmountServiceProtocol

    init(asset: Asset, action: EarnType, service: any BsvHomeAmountServiceProtocol) {
        self.asset = asset
        self.action = action
        self.service = service
    }

    var provider: DelegationValidator {
        switch action {
        case let .deposit(provider): provider
        case let .withdraw(delegation): delegation.validator
        }
    }

    var providerTitle: String {
        Localized.Common.provider
    }

    var title: String {
        switch action {
        case .deposit: Localized.Wallet.deposit
        case .withdraw: Localized.Wallet.withdraw
        }
    }

    var bsvHomeAmountType: BsvHomeAmountType {
        service.earnAmountType(earnType: action.map())
    }

    func makeTransferData(value: BigInt, useMaxAmount: Bool) async throws -> BsvHomeTransferData {
        try await service.earnTransferData(asset: asset.map(), earnType: action.map(), value: value, useMaxAmount: useMaxAmount)
    }
}
