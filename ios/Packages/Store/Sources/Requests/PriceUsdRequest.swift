// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct PriceUsdRequest: DatabaseQueryable {
    public var assetId: AssetId

    public init(assetId: AssetId) {
        self.assetId = assetId
    }

    public func fetch(_ db: Database) throws -> Double? {
        guard Chain.productionCases.contains(assetId.chain) else { return nil }
        return try PriceRecord
            .filter(PriceRecord.Columns.assetId == assetId.identifier)
            .fetchOne(db)?
            .priceUsd
    }
}

extension PriceUsdRequest: Equatable {}
