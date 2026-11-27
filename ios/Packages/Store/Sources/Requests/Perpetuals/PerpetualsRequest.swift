// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct PerpetualsRequest: DatabaseQueryable {
    static let defaultQueryLimit = 100

    public let searchQuery: String

    private let environment: BsvHomeRuntimeEnvironment

    public init(searchQuery: String = "", environment: BsvHomeRuntimeEnvironment = .current) {
        self.searchQuery = searchQuery
        self.environment = environment
    }

    public func fetch(_ db: Database) throws -> [PerpetualData] {
        guard environment.allowedAssetIds() == nil else { return [] }
        var request = PerpetualRecord.including(required: PerpetualRecord.asset)

        if !searchQuery.isEmpty {
            request = request.filter(
                PerpetualRecord.Columns.name.like("%%\(searchQuery)%%") ||
                    TableAlias(name: AssetRecord.databaseTableName)[AssetRecord.Columns.symbol].like("%%\(searchQuery)%%"),
            )
        }

        return try request
            .order(PerpetualRecord.Columns.volume24h.desc)
            .limit(Self.defaultQueryLimit)
            .asRequest(of: PerpetualInfo.self)
            .fetchAll(db)
            .map { $0.mapToPerpetualData() }
    }
}

extension PerpetualsRequest: Equatable {}
