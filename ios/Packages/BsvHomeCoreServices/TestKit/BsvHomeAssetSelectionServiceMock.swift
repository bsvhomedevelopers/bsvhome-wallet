// Copyright (c). Gem Wallet. All rights reserved.

public import struct BsvHomeCore.Account
import Foundation
import typealias BsvHomeCore.Asset
import typealias BsvHomeCore.AssetBasic
import typealias BsvHomeCore.AssetId
import typealias BsvHomeCore.Chain
import typealias BsvHomeCore.Currency
import enum BsvHomeCore.BsvHomeAssetAction
import protocol BsvHomeCore.BsvHomeAssetSelectionServiceProtocol
import enum BsvHomeCore.BsvHomeNftItem
import struct BsvHomeCore.BsvHomeSelectAssetFlow
import enum BsvHomeCore.BsvHomeSelectAssetType
import struct BsvHomeCore.BsvHomeWalletSearchLimits
import struct BsvHomeCore.Wallet
import enum BsvHomeCore.WalletType
import typealias BsvHomeCore.NftData
import enum BsvHomeCore.BsvHomeSearchScope
import Primitives

public final class BsvHomeAssetSelectionServiceMock: BsvHomeAssetSelectionServiceProtocol, @unchecked Sendable {
    private let assets: [AssetBasic]
    private let error: Error?
    private let onSetAssetsEnabled: (@Sendable ([AssetId], Bool) -> Void)?
    private let onSetAssetPinned: (@Sendable (AssetId, Bool) -> Void)?

    public init(
        assets: [AssetBasic] = [],
        error: Error? = nil,
        onSetAssetsEnabled: (@Sendable ([AssetId], Bool) -> Void)? = nil,
        onSetAssetPinned: (@Sendable (AssetId, Bool) -> Void)? = nil,
    ) {
        self.assets = assets
        self.error = error
        self.onSetAssetsEnabled = onSetAssetsEnabled
        self.onSetAssetPinned = onSetAssetPinned
    }

    public var perpetualsShown = true
    public var tokensSupported = true
    public var nftSearchItems: [BsvHomeNftItem] = []
    public var filterChainsResult: [BsvHomeCore.Chain] = []
    public private(set) var pinnedPerpetuals: [(perpetualId: String, pinned: Bool)] = []

    public func flow(selectType: BsvHomeSelectAssetType) -> BsvHomeSelectAssetFlow {
        selectType.flow()
    }

    public func walletSearchLimits(query _: String) -> BsvHomeWalletSearchLimits {
        BsvHomeWalletSearchLimits(assets: 12, fetch: 13, perpetuals: 3, nfts: 3, results: 100)
    }

    public func getCurrency() -> Currency {
        Primitives.Currency.usd.rawValue
    }

    public func showPerpetuals(walletType _: BsvHomeCore.WalletType, chains _: [BsvHomeCore.Chain]) -> Bool {
        perpetualsShown
    }

    public func searchCollections(data _: [NftData], query _: String) -> [BsvHomeNftItem] {
        nftSearchItems
    }

    public func supportsTokens(wallet _: BsvHomeCore.Wallet?) -> Bool {
        tokensSupported
    }

    public func filterChains(wallet _: BsvHomeCore.Wallet) -> [BsvHomeCore.Chain] {
        filterChainsResult
    }

    public func search(query _: String, scope _: BsvHomeSearchScope) async throws -> Bool {
        if let error { throw error }
        return true
    }

    public func searchKey(query: String, scope: BsvHomeSearchScope) -> String {
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines)
        switch scope {
        case .all: return query
        case let .list(id): return query.isEmpty ? "tag:\(id)" : query
        }
    }

    public func setAssetPinned(assetId: AssetId, pinned: Bool) async throws {
        onSetAssetPinned?(assetId, pinned)
    }

    public func setPerpetualPinned(perpetualId: String, pinned: Bool) async throws {
        pinnedPerpetuals.append((perpetualId, pinned))
    }

    public func searchAssets(query _: String) async throws -> [AssetBasic] {
        if let error { throw error }
        return assets
    }

    public func setAssetsEnabled(assetIds: [AssetId], enabled: Bool) async throws {
        onSetAssetsEnabled?(assetIds, enabled)
    }

    public func addRecent(action _: BsvHomeAssetAction, asset _: Asset) async throws {}

    public func setPriceAlert(assetId _: AssetId, enabled _: Bool) async throws {}
}
