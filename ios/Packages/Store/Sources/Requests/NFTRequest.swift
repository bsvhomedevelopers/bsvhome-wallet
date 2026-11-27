// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public enum NFTFilter: Sendable, Hashable {
    case all
    case unverified
    case collection(id: String)
}

public struct NFTRequest: DatabaseQueryable {
    private let walletId: WalletId
    private let filter: NFTFilter
    private let scopePolicy: AssetQueryScope

    public init(walletId: WalletId, filter: NFTFilter, allowedChains: [Chain]? = nil, environment: BsvHomeRuntimeEnvironment = .current) {
        self.walletId = walletId
        self.filter = filter
        scopePolicy = AssetQueryScope(environment: environment, allowedChains: allowedChains)
    }

    public func fetch(_ db: Database) throws -> [NFTData] {
        guard scopePolicy.assetIds == nil else { return [] }
        var request = NFTCollectionRecord
            .filter(scopePolicy.chains.contains(NFTCollectionRecord.Columns.chain))
            .including(
                all: NFTCollectionRecord.assets
                    .joining(
                        required: NFTAssetRecord.assetAssociations
                            .filter(NFTAssetAssociationRecord.Columns.walletId == walletId.id),
                    ),
            )
            .distinct()
            .asRequest(of: NFTCollectionRecordInfo.self)

        switch filter {
        case .all: break
        case .unverified: request = request.filter(NFTCollectionRecord.Columns.status != VerificationStatus.verified.rawValue)
        case let .collection(id): request = request.filter(NFTCollectionRecord.Columns.id == id)
        }

        return try request
            .fetchAll(db)
            .map { $0.mapToNFTData() }
            .filter(\.assets.isNotEmpty)
            .sorted(using: [
                KeyPathComparator(\.assets.count, order: .reverse),
                KeyPathComparator(\.collection.name),
            ])
    }
}

extension NFTRequest: Equatable {}
