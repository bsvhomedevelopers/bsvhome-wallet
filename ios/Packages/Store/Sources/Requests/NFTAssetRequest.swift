// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct NFTAssetRequest: DatabaseQueryable {
    private let walletId: WalletId
    private let assetId: NFTAssetId
    private let environment: BsvHomeRuntimeEnvironment

    public init(walletId: WalletId, assetId: NFTAssetId, environment: BsvHomeRuntimeEnvironment = .current) {
        self.walletId = walletId
        self.assetId = assetId
        self.environment = environment
    }

    public func fetch(_ db: Database) throws -> NFTAssetDetails {
        guard environment.allowedAssetIds() == nil else {
            throw AnyError("NFT asset unavailable in this environment")
        }
        guard let info = try NFTAssetRecord
            .filter(NFTAssetRecord.Columns.id == assetId.identifier)
            .including(required: NFTAssetRecord.collection.forKey("collection"))
            .including(all: NFTAssetRecord.assetAssociations.filter(NFTAssetAssociationRecord.Columns.walletId == walletId.id).forKey("associations"))
            .asRequest(of: NFTAssetRecordInfo.self)
            .fetchOne(db)
        else {
            throw AnyError("NFT asset not found: \(assetId.identifier)")
        }
        return info.mapToDetails()
    }
}

extension NFTAssetRequest: Equatable {}
