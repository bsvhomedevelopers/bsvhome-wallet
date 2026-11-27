// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct PriceAlertsRequest: DatabaseQueryable {
    public let assetId: AssetId?
    private let scopePolicy: AssetQueryScope

    public init(assetId: AssetId? = nil, environment: BsvHomeRuntimeEnvironment = .current) {
        self.assetId = assetId
        scopePolicy = AssetQueryScope(environment: environment)
    }

    public func fetch(_ db: Database) throws -> [PriceAlertData] {
        var request = AssetRecord
            .including(required: AssetRecord.priceAlert)
            .including(optional: AssetRecord.price)
            .filter(AssetRecord.Columns.rank >= 0)
            .filter(scopePolicy.filter(assetId: AssetRecord.Columns.id, chain: AssetRecord.Columns.chain))
            .order(AssetRecord.Columns.rank.desc)

        if let assetId {
            request = request.filter(AssetRecord.Columns.id == assetId.identifier)
        }

        return try request
            .asRequest(of: PriceAlertInfo.self)
            .fetchAll(db)
            .map(\.data)
    }
}

struct PriceAlertInfo: FetchableRecord, Codable {
    var asset: AssetRecord
    var price: PriceRecord?
    var priceAlert: PriceAlertRecord
}

extension PriceAlertsRequest: Equatable {}

extension PriceAlertInfo {
    var data: PriceAlertData {
        PriceAlertData(
            asset: asset.mapToAsset(),
            price: price?.mapToPrice(),
            priceAlert: priceAlert.map(),
        )
    }
}
