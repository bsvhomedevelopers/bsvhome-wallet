// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct WalletsRequest: DatabaseQueryable {
    private let isPinned: Bool?
    private let allowedChains: [Chain]
    private let environment: BsvHomeRuntimeEnvironment

    public init(isPinned: Bool?, allowedChains: [Chain]? = nil, environment: BsvHomeRuntimeEnvironment = .current) {
        self.isPinned = isPinned
        self.allowedChains = AssetQueryScope(environment: environment, allowedChains: allowedChains).chains.compactMap(Chain.init(rawValue:))
        self.environment = environment
    }

    public func fetch(_ db: Database) throws -> [Wallet] {
        var request = WalletRecord
            .including(all: WalletRecord.accounts.filter(allowedChains.map(\.rawValue).contains(AccountRecord.Columns.chain)))
        if let isPinned {
            request = request.filter(WalletRecord.Columns.isPinned == isPinned)
        }
        return try request
            .asRequest(of: WalletRecordInfo.self)
            .fetchAll(db)
            .filter { $0.wallet.id.chain.map(allowedChains.contains) ?? true }
            .map { $0.mapToWallet().map() }
            .filter { environment.allowsWallet(wallet: $0) }
            .map { environment.visibleWallet(wallet: $0).map() }
    }
}

extension WalletsRequest: Equatable {}
