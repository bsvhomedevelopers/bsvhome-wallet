// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct TransactionRequest: DatabaseQueryable {
    private let walletId: WalletId
    private let recordId: UInt64
    private let scopePolicy: AssetQueryScope

    public init(walletId: WalletId, recordId: UInt64, environment: BsvHomeRuntimeEnvironment = .current) {
        self.walletId = walletId
        self.recordId = recordId
        scopePolicy = AssetQueryScope(environment: environment)
    }

    public func fetch(_ db: Database) throws -> TransactionExtended {
        let request = TransactionRecord
            .filter(TransactionRecord.Columns.walletId == walletId.id)
            .filter(TransactionRecord.Columns.id == recordId)
            .filter(scopePolicy.filter(assetId: TransactionRecord.Columns.assetId, chain: TransactionRecord.Columns.chain))
        guard let transaction = try TransactionsRequest.fetch(db, request: request).first else {
            throw RecordError.recordNotFound(databaseTableName: TransactionRecord.databaseTableName, key: [:])
        }
        return transaction
    }
}

extension TransactionRequest: Equatable {}
