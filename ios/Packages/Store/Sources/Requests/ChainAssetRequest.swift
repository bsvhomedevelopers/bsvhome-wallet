// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct ChainAssetRequest: DatabaseQueryable {
    public var assetRequest: AssetRequest
    public var feeAssetRequest: AssetRequest

    public init(walletId: WalletId, assetId: AssetId, environment: BsvHomeRuntimeEnvironment = .current) {
        assetRequest = AssetRequest(walletId: walletId, assetId: assetId, environment: environment)
        feeAssetRequest = AssetRequest(walletId: walletId, assetId: assetId.chain.assetId, environment: environment)
    }

    public func fetch(_ db: Database) throws -> ChainAssetData {
        if assetRequest.assetId == feeAssetRequest.assetId {
            let assetData = try assetRequest.fetch(db)
            return ChainAssetData(
                assetData: assetData,
                feeAssetData: assetData,
            )
        }
        return try ChainAssetData(
            assetData: assetRequest.fetch(db),
            feeAssetData: feeAssetRequest.fetch(db),
        )
    }
}

extension ChainAssetRequest: Equatable {}
