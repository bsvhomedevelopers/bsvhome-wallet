// Copyright (c). Gem Wallet. All rights reserved.

public import struct BsvHomeCore.Account
import typealias BsvHomeCore.Asset
import typealias BsvHomeCore.AssetId
import typealias BsvHomeCore.Chain
import protocol BsvHomeCore.BsvHomeAddAssetServiceProtocol
import struct BsvHomeCore.BlockExplorerLink
import struct BsvHomeCore.Wallet
import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit

public final class BsvHomeAddAssetServiceMock: BsvHomeAddAssetServiceProtocol, @unchecked Sendable {
    private let chains: [Primitives.Chain]
    private let asset: Primitives.Asset

    public init(chains: [Primitives.Chain] = [.ethereum], asset: Primitives.Asset = .mock()) {
        self.chains = chains
        self.asset = asset
    }

    public func chains(wallet _: Wallet) -> [Chain] { chains.map(\.rawValue) }
    public func defaultChain(chains: [Chain]) -> Chain? { chains.first }
    public func matchingChains(chains: [Chain], query: String) -> [Chain] { chains }
    public func tokenUrl(chain: Chain, tokenId: String) -> BsvHomeCore.BlockExplorerLink? { nil }
    public func token(chain: Chain, address: String) async throws -> Asset { asset.map() }
    public func add(wallet: Wallet, assetId: AssetId) async throws {}
}
