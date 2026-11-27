// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct ConnectionsRequest: DatabaseQueryable {
    private let allowedChains: [Chain]
    private let environment: BsvHomeRuntimeEnvironment

    public init(allowedChains: [Chain]? = nil, environment: BsvHomeRuntimeEnvironment = .current) {
        self.allowedChains = AssetQueryScope(environment: environment, allowedChains: allowedChains).chains.compactMap(Chain.init(rawValue:))
        self.environment = environment
    }

    public func fetch(_ db: Database) throws -> [WalletConnection] {
        try WalletRecord
            .including(required: WalletRecord.connection)
            .including(all: WalletRecord.accounts.filter(allowedChains.map(\.rawValue).contains(AccountRecord.Columns.chain)))
            .asRequest(of: WalletConnectionInfo.self)
            .fetchAll(db)
            .filter { $0.wallet.wallet.id.chain.map(allowedChains.contains) ?? true }
            .map { $0.mapToWalletConnection() }
            .filter { environment.allowsWallet(wallet: $0.wallet.map()) }
            .map { connection in
                WalletConnection(session: connection.session, wallet: environment.visibleWallet(wallet: connection.wallet.map()).map())
            }
    }
}

extension ConnectionsRequest: Equatable {}
