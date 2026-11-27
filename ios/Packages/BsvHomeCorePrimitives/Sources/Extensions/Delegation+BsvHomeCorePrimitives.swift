// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import struct BsvHomeCore.Delegation
import struct BsvHomeCore.DelegationBase
import enum BsvHomeCore.EarnType
import struct BsvHomeCore.RedelegateData
import enum BsvHomeCore.StakeType
import Primitives

public extension Primitives.DelegationBase {
    init(core: BsvHomeCore.DelegationBase) {
        self.init(
            assetId: Primitives.AssetId(core: core.assetId),
            state: core.state.map(),
            balance: BigInt(core.balance),
            shares: BigInt(core.shares),
            rewards: BigInt(core.rewards),
            completionDate: core.completionDate,
            delegationId: core.delegationId,
            validatorId: core.validatorId,
        )
    }

    func map() -> BsvHomeCore.DelegationBase {
        BsvHomeCore.DelegationBase(
            assetId: assetId.identifier,
            state: state.map(),
            balance: balance.magnitude,
            shares: shares.magnitude,
            rewards: rewards.magnitude,
            completionDate: completionDate,
            delegationId: delegationId,
            validatorId: validatorId,
        )
    }
}

public extension Primitives.Delegation {
    init(core: BsvHomeCore.Delegation) {
        self.init(
            base: Primitives.DelegationBase(core: core.base),
            validator: core.validator.map(),
            price: .none,
        )
    }

    func map() -> BsvHomeCore.Delegation {
        BsvHomeCore.Delegation(base: base.map(), validator: validator.map())
    }
}

public extension Primitives.RedelegateData {
    init(core: BsvHomeCore.RedelegateData) {
        self.init(
            delegation: Primitives.Delegation(core: core.delegation),
            toValidator: core.toValidator.map(),
        )
    }

    func map() -> BsvHomeCore.RedelegateData {
        BsvHomeCore.RedelegateData(delegation: delegation.map(), toValidator: toValidator.map())
    }
}

public extension Primitives.StakeType {
    func map() -> BsvHomeCore.StakeType {
        switch self {
        case let .stake(validator): .stake(validator.map())
        case let .unstake(delegation): .unstake(delegation.map())
        case let .redelegate(data): .redelegate(data.map())
        case let .rewards(validators): .rewards(validators.map { $0.map() })
        case let .withdraw(delegation): .withdraw(delegation.map())
        case let .freeze(resource): .freeze(resource.map())
        case let .unfreeze(resource): .unfreeze(resource.map())
        }
    }
}

public extension Primitives.EarnType {
    func map() -> BsvHomeCore.EarnType {
        switch self {
        case let .deposit(validator): .deposit(validator.map())
        case let .withdraw(delegation): .withdraw(delegation.map())
        }
    }
}
