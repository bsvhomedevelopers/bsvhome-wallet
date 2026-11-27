// Copyright (c). Gem Wallet. All rights reserved.

import protocol BsvHomeCore.BsvHomeAddAssetServiceProtocol
import Primitives

public extension BsvHomeAddAssetServiceProtocol {
    func chains(wallet: Wallet) -> [Chain] {
        chains(wallet: wallet.map()).map { Chain(core: $0) }
    }

    func defaultChain(chains: [Chain]) -> Chain? {
        defaultChain(chains: chains.map(\.rawValue)).map { Chain(core: $0) }
    }

    func tokenUrl(chain: Chain, tokenId: String) -> BlockExplorerLink? {
        tokenUrl(chain: chain.rawValue, tokenId: tokenId).map { $0.map() }
    }

    func token(chain: Chain, address: String) async throws -> Asset {
        try await token(chain: chain.rawValue, address: address).map()
    }

    func add(wallet: Wallet, assetId: AssetId) async throws {
        try await add(wallet: wallet.map(), assetId: assetId.identifier)
    }
}
