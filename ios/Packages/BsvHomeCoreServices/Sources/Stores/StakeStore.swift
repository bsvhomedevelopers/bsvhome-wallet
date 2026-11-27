// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import typealias BsvHomeCore.AssetId
import struct BsvHomeCore.DelegationBase
import struct BsvHomeCore.DelegationValidator
import typealias BsvHomeCore.StakeProviderType
import protocol BsvHomeCore.BsvHomeStakeStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreStakeStore: BsvHomeStakeStore, @unchecked Sendable {
    private let store: StakeStore

    public init(store: StakeStore) {
        self.store = store
    }

    public func getApr(assetId: BsvHomeCore.AssetId, providerType: BsvHomeCore.StakeProviderType) async throws -> Double? {
        let assetId = try Primitives.AssetId(id: assetId)
        switch providerType.map() {
        case .stake: return try store.getStakeApr(assetId: assetId)
        case .earn: return try store.getEarnApr(assetId: assetId)
        }
    }

    public func getValidators(assetId: BsvHomeCore.AssetId, providerType: BsvHomeCore.StakeProviderType) async throws -> [BsvHomeCore.DelegationValidator] {
        try store.getValidators(assetId: Primitives.AssetId(id: assetId), providerType: providerType.map()).map { $0.map() }
    }

    public func saveValidators(validators: [BsvHomeCore.DelegationValidator]) async throws {
        try store.updateValidators(validators.map { $0.map() })
    }

    public func deactivateValidators(assetId: BsvHomeCore.AssetId, validatorIds: [String]) async throws {
        try store.deactivateValidators(assetId: Primitives.AssetId(id: assetId), validatorIds: validatorIds)
    }

    public func getDelegationIds(walletId: String, assetId: BsvHomeCore.AssetId, providerType: BsvHomeCore.StakeProviderType) async throws -> [String] {
        try store.getDelegations(walletId: WalletId.from(id: walletId), assetId: Primitives.AssetId(id: assetId), providerType: providerType.map()).map(\.id)
    }

    public func updateDelegations(walletId: String, delegations: [BsvHomeCore.DelegationBase], deleteIds: [String]) async throws {
        try store.updateAndDelete(
            walletId: WalletId.from(id: walletId),
            delegations: delegations.map { Primitives.DelegationBase(core: $0) },
            deleteIds: deleteIds,
        )
    }

}
