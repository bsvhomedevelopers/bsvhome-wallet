// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct AssetRequestOptional: DatabaseQueryable {
    public var assetId: AssetId?
    private let walletId: WalletId
    private let scopePolicy: AssetQueryScope

    public init(
        walletId: WalletId,
        assetId: AssetId?,
        environment: BsvHomeRuntimeEnvironment = .current,
    ) {
        self.walletId = walletId
        self.assetId = assetId
        scopePolicy = AssetQueryScope(environment: environment)
    }

    public func fetch(_ db: Database) throws -> AssetData? {
        try AssetRecord.assetInfoRequest(walletId: walletId, assetId: assetId, scopePolicy: scopePolicy)
            .fetchOne(db)?
            .assetData
    }
}

extension AssetRequestOptional: Equatable {}
