// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.Account
import class BsvHomeCore.BsvHomeChainService
import Primitives
import ReownWalletKit

extension Primitives.Account {
    func blockchain(chainService: BsvHomeChainService) -> WalletConnectUtils.Account? {
        guard let blockchain = chain.blockchain(chainService: chainService) else {
            return .none
        }
        return WalletConnectUtils.Account(blockchain: blockchain, address: address)
    }

    public func mapToBsvHome() -> BsvHomeCore.Account {
        BsvHomeCore.Account(
            chain: chain.rawValue,
            address: address,
            derivationPath: derivationPath,
            extendedPublicKey: extendedPublicKey,
        )
    }
}
