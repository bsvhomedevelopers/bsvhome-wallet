// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct PerpetualRequest: DatabaseQueryable {
    public let assetId: AssetId

    private let environment: BsvHomeRuntimeEnvironment

    public init(assetId: AssetId, environment: BsvHomeRuntimeEnvironment = .current) {
        self.assetId = assetId
        self.environment = environment
    }

    public func fetch(_ db: Database) throws -> PerpetualData {
        guard environment.allowedAssetIds() == nil else { return .empty }
        let result = try PerpetualRecord
            .including(required: PerpetualRecord.asset)
            .filter(PerpetualRecord.Columns.assetId == assetId.identifier)
            .asRequest(of: PerpetualInfo.self)
            .fetchOne(db)?
            .mapToPerpetualData()

        return result ?? .empty
    }
}

// MARK: - Models Extensions

extension PerpetualRequest: Equatable {}

public extension PerpetualData {
    static var empty: PerpetualData {
        PerpetualData(
            perpetual: Perpetual(
                id: PerpetualId(provider: .hypercore, symbol: ""),
                name: "",
                provider: .hypercore,
                assetId: AssetId(chain: .bitcoin, tokenId: nil),
                identifier: "",
                price: .zero,
                pricePercentChange24h: .zero,
                openInterest: .zero,
                volume24h: .zero,
                funding: .zero,
                maxLeverage: 1,
                isIsolatedOnly: false,
            ),
            asset: Asset(id: .init(chain: .bitcoin, tokenId: .none), name: "", symbol: "", decimals: 0, type: .native),
            metadata: PerpetualMetadata(isPinned: false),
        )
    }
}
