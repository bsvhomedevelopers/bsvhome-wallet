// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct TransactionsRequest: DatabaseQueryable {
    private let walletId: WalletId
    private let type: TransactionsRequestType
    private let limit: Int?
    private let scopePolicy: AssetQueryScope

    public var filters: [TransactionsRequestFilter] = []

    public init(
        walletId: WalletId,
        type: TransactionsRequestType,
        filters: [TransactionsRequestFilter] = [],
        limit: Int? = nil,
        allowedChains: [Chain]? = nil,
        environment: BsvHomeRuntimeEnvironment = .current,
    ) {
        self.walletId = walletId
        self.type = type
        self.filters = filters
        self.limit = limit
        scopePolicy = AssetQueryScope(environment: environment, allowedChains: allowedChains)
    }

    public func fetch(_ db: Database) throws -> [TransactionExtended] {
        let request = Self.query(walletId: walletId, type: type, filters: filters, scopePolicy: scopePolicy)
        return try Self.fetch(db, request: limit.map { request.limit($0) } ?? request)
    }

    public static func fetch(
        _ db: Database,
        type: TransactionsRequestType,
        filters: [TransactionsRequestFilter],
        walletId: WalletId,
        limit: Int? = nil,
        allowedChains: [Chain]? = nil,
        environment: BsvHomeRuntimeEnvironment = .current,
    ) throws -> [TransactionExtended] {
        let request = query(walletId: walletId, type: type, filters: filters, scopePolicy: AssetQueryScope(environment: environment, allowedChains: allowedChains))
        return try fetch(db, request: limit.map { request.limit($0) } ?? request)
    }

    static func fetch(_ db: Database, request: QueryInterfaceRequest<TransactionRecord>) throws -> [TransactionExtended] {
        try request
            .including(required: TransactionRecord.asset)
            .including(required: TransactionRecord.feeAsset)
            .including(optional: TransactionRecord.price)
            .including(optional: TransactionRecord.feePrice)
            .including(all: TransactionRecord.assets)
            .including(all: TransactionRecord.prices)
            .including(optional: TransactionRecord.fromAddress)
            .including(optional: TransactionRecord.toAddress)
            .order(TransactionRecord.Columns.date.desc)
            .asRequest(of: TransactionInfo.self)
            .fetchAll(db)
            .map { try $0.mapToTransactionExtended() }
    }

    static func query(
        walletId: WalletId,
        type: TransactionsRequestType,
        filters: [TransactionsRequestFilter],
        scopePolicy: AssetQueryScope,
    ) -> QueryInterfaceRequest<TransactionRecord> {
        let states = states(type: type)
        var request = TransactionRecord
            .filter(TransactionRecord.Columns.walletId == walletId.id)
            .filter(states.contains(TransactionRecord.Columns.state))
            .filter(scopePolicy.filter(assetId: TransactionRecord.Columns.assetId, chain: TransactionRecord.Columns.chain))
            .distinct()

        switch type {
        case let .asset(assetId):
            request = request.joining(required: TransactionRecord.assetsAssociation.filter(TransactionAssetAssociationRecord.Columns.assetId == assetId.identifier))
        case let .transaction(id):
            request = request.filter(TransactionRecord.Columns.transactionId == id)
        case .all, .pending:
            break
        }

        for filter in filters {
            request = Self.applyFilter(request: request, filter)
        }

        return request
    }
}

// MARK: - Private

extension TransactionsRequest {
    static func applyFilter(request: QueryInterfaceRequest<TransactionRecord>, _ filter: TransactionsRequestFilter) -> QueryInterfaceRequest<TransactionRecord> {
        switch filter {
        case let .chains(chains):
            guard !chains.isEmpty else { return request }
            return request.filter(chains.contains(TransactionRecord.Columns.chain))
        case let .types(types):
            guard !types.isEmpty else { return request }
            return request.filter(types.contains(TransactionRecord.Columns.type))
        case let .assetRankGreaterThan(rank):
            return request.joining(required: TransactionRecord.asset.filter(AssetRecord.Columns.rank > rank))
        }
    }

    private static func states(type: TransactionsRequestType) -> [String] {
        switch type {
        case .pending:
            [TransactionState.pending, TransactionState.inTransit].map(\.rawValue)
        case .all, .asset, .transaction:
            TransactionState.allCases.map(\.rawValue)
        }
    }
}

extension TransactionsRequest: Equatable {}
