// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct AssetFiatValuesRequest: DatabaseQueryable, Equatable {
    public enum PriceBasis: Sendable {
        case selectedCurrency
        case usd
    }

    public var walletId: WalletId
    public let priceBasis: PriceBasis
    private let scopePolicy: AssetQueryScope

    public init(walletId: WalletId, priceBasis: PriceBasis = .selectedCurrency, environment: BsvHomeRuntimeEnvironment = .current) {
        self.walletId = walletId
        self.priceBasis = priceBasis
        scopePolicy = AssetQueryScope(environment: environment)
    }

    public func fetch(_ db: Database) throws -> [AssetFiatValue] {
        try AssetRecord
            .including(optional: AssetRecord.price)
            .including(optional: AssetRecord.balance)
            .filter(AssetRecord.Columns.rank >= 0)
            .filter(scopePolicy.filter(assetId: AssetRecord.Columns.id, chain: AssetRecord.Columns.chain))
            .joining(required: AssetRecord.balance
                .filter(BalanceRecord.Columns.walletId == walletId.id)
                .filter(BalanceRecord.Columns.isEnabled == true))
            .asRequest(of: AssetRecordInfoMinimal.self)
            .fetchAll(db)
            .map {
                AssetFiatValue(
                    amount: $0.balance.totalAmount,
                    price: (priceBasis == .usd ? $0.price?.priceUsd : $0.price?.price) ?? 0,
                    priceChangePercentage24h: $0.price?.priceChangePercentage24h ?? 0,
                )
            }
    }
}
