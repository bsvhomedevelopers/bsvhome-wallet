// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct TransactionsCountRequest: DatabaseQueryable {
    public var walletId: WalletId
    private let type: TransactionsRequestType
    private let filters: [TransactionsRequestFilter]
    private let scopePolicy: AssetQueryScope

    public init(
        walletId: WalletId,
        type: TransactionsRequestType,
        filters: [TransactionsRequestFilter] = [],
        allowedChains: [Chain]? = nil,
        environment: BsvHomeRuntimeEnvironment = .current,
    ) {
        self.walletId = walletId
        self.type = type
        self.filters = filters
        scopePolicy = AssetQueryScope(environment: environment, allowedChains: allowedChains)
    }

    public func fetch(_ db: Database) throws -> Int {
        try TransactionsRequest.query(walletId: walletId, type: type, filters: filters, scopePolicy: scopePolicy)
            .fetchCount(db)
    }
}

extension TransactionsCountRequest: Equatable {}
