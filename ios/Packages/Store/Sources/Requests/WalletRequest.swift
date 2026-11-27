// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct WalletRequest: DatabaseQueryable {
    public var walletId: WalletId
    private let allowedChains: [Chain]
    private let environment: BsvHomeRuntimeEnvironment

    public init(walletId: WalletId, allowedChains: [Chain]? = nil, environment: BsvHomeRuntimeEnvironment = .current) {
        self.walletId = walletId
        self.allowedChains = AssetQueryScope(environment: environment, allowedChains: allowedChains).chains.compactMap(Chain.init(rawValue:))
        self.environment = environment
    }

    public func fetch(_ db: Database) throws -> Wallet {
        guard walletId.chain.map(allowedChains.contains) ?? true else {
            throw AnyError("wallet not available: \(walletId.id)")
        }
        guard let wallet = try WalletRecord
            .including(all: WalletRecord.accounts.filter(allowedChains.map(\.rawValue).contains(AccountRecord.Columns.chain)))
            .asRequest(of: WalletRecordInfo.self)
            .filter(WalletRecord.Columns.id == walletId.id)
            .fetchOne(db)?
            .mapToWallet()
        else {
            throw AnyError("wallet not found: \(walletId.id)")
        }
        let coreWallet = wallet.map()
        guard environment.allowsWallet(wallet: coreWallet) else {
            throw AnyError("wallet not available: \(walletId.id)")
        }
        return environment.visibleWallet(wallet: coreWallet).map()
    }
}

extension WalletRequest: Equatable {}
