// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct PriceRequest: DatabaseQueryable {
    public var assetId: AssetId
    private let scopePolicy: AssetQueryScope

    public init(assetId: AssetId, environment: BsvHomeRuntimeEnvironment = .current) {
        self.assetId = assetId
        scopePolicy = AssetQueryScope(environment: environment)
    }

    public func fetch(_ db: Database) throws -> PriceData? {
        try AssetRecord
            .including(optional: AssetRecord.price)
            .including(all: AssetRecord.priceAlerts)
            .including(all: AssetRecord.links)
            .filter(AssetRecord.Columns.id == assetId.identifier)
            .filter(scopePolicy.filter(assetId: AssetRecord.Columns.id, chain: AssetRecord.Columns.chain))
            .asRequest(of: PriceRecordInfo.self)
            .fetchOne(db)
            .map(\.priceData)
    }
}

extension PriceRequest: Equatable {}
