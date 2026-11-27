// Copyright (c). Gem Wallet. All rights reserved.

import protocol BsvHomeCore.BsvHomeAmountServiceProtocol
import enum BsvHomeCore.BsvHomeAmountType
import BigInt
import BsvHomeCorePrimitives
import Primitives
import struct BsvHomeCore.BsvHomeTransferData

public enum AmountDataProvider: AmountDataProvidable, @unchecked Sendable {
    case transfer(AmountTransferViewModel)
    case stake(AmountStakeViewModel)
    case perpetual(AmountPerpetualViewModel)
    case earn(AmountEarnViewModel)

    static func make(from input: AmountInput, service: any BsvHomeAmountServiceProtocol) -> AmountDataProvider {
        switch input.type {
        case let .transfer(recipient):
            .transfer(AmountTransferViewModel(asset: input.asset, transfer: .send(payment: recipient), service: service))
        case .deposit:
            .transfer(AmountTransferViewModel(asset: input.asset, transfer: .deposit, service: service))
        case .withdraw:
            .transfer(AmountTransferViewModel(asset: input.asset, transfer: .withdraw, service: service))
        case let .stake(stakeType):
            .stake(AmountStakeViewModel(asset: input.asset, type: stakeType, service: service))
        case let .perpetual(action):
            .perpetual(AmountPerpetualViewModel(asset: input.asset, action: action, service: service))
        case let .earn(earnType):
            .earn(AmountEarnViewModel(asset: input.asset, action: earnType, service: service))
        }
    }

    var asset: Asset {
        provider.asset
    }

    var title: String {
        provider.title
    }

    var bsvHomeAmountType: BsvHomeAmountType {
        provider.bsvHomeAmountType
    }

    var prefilledAmount: String? {
        provider.prefilledAmount
    }

    func makeTransferData(value: BigInt, useMaxAmount: Bool) async throws -> BsvHomeTransferData {
        try await provider.makeTransferData(value: value, useMaxAmount: useMaxAmount)
    }
}

// MARK: - Private

extension AmountDataProvider {
    private var provider: any AmountDataProvidable {
        switch self {
        case let .transfer(provider): provider
        case let .stake(provider): provider
        case let .perpetual(provider): provider
        case let .earn(provider): provider
        }
    }
}
