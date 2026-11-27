// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct AssetRequest: DatabaseQueryable {
    public var assetId: AssetId
    private let walletId: WalletId
    private let scopePolicy: AssetQueryScope

    public init(
        walletId: WalletId,
        assetId: AssetId,
        environment: BsvHomeRuntimeEnvironment = .current,
    ) {
        self.walletId = walletId
        self.assetId = assetId
        scopePolicy = AssetQueryScope(environment: environment)
    }

    public func fetch(_ db: Database) throws -> AssetData {
        guard let result = try AssetRecord.assetInfoRequest(walletId: walletId, assetId: assetId, scopePolicy: scopePolicy)
            .fetchOne(db)?
            .assetData
        else {
            throw AnyError("Asset not found: \(assetId.identifier)")
        }
        return result
    }
}

extension AssetRecord {
    static func assetInfoRequest(
        walletId: WalletId,
        assetId: AssetId?,
        scopePolicy: AssetQueryScope? = nil,
    ) -> QueryInterfaceRequest<AssetRecordInfo> {
        AssetRecord
            .including(optional: AssetRecord.price)
            .including(optional: AssetRecord.balance.filter(BalanceRecord.Columns.walletId == walletId.id))
            .including(optional: AssetRecord.account.filter(AccountRecord.Columns.walletId == walletId.id))
            .including(all: AssetRecord.priceAlerts)
            .filter(AssetRecord.Columns.id == assetId?.identifier)
            .filter(scopePolicy?.filter(assetId: AssetRecord.Columns.id, chain: AssetRecord.Columns.chain) ?? true.sqlExpression)
            .asRequest(of: AssetRecordInfo.self)
    }
}
