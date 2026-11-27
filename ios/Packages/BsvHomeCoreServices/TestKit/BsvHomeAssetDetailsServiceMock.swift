// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import typealias BsvHomeCore.Asset
import typealias BsvHomeCore.AssetId
import struct BsvHomeCore.AssetMetaData
import typealias BsvHomeCore.BannerEvent
import struct BsvHomeCore.BlockExplorerLink
import func BsvHomeCore.bsvColdWalletCanSend
import typealias BsvHomeCore.Chain
import typealias BsvHomeCore.Deeplink
import struct BsvHomeCore.BsvHomeAssetBalance
import protocol BsvHomeCore.BsvHomeAssetDetailsServiceProtocol
import struct BsvHomeCore.BsvHomeAssetDetailsState
import enum BsvHomeCore.BsvHomeAssetNetworkDestination
import struct BsvHomeCore.BsvHomeAssetRefreshFailure
import enum BsvHomeCore.BsvHomeBannerAction
import struct BsvHomeCore.BsvHomeBannerContent
import struct BsvHomeCore.BsvHomeBannerKey
import struct BsvHomeCore.BsvHomeBsvAddressSnapshot
import enum BsvHomeCore.BsvHomeHeaderActions
import struct BsvHomeCore.BsvHomeHeaderButton
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import struct BsvHomeCore.BsvHomeSwapPairSuggestion
import struct BsvHomeCore.PriceAlert
import enum BsvHomeCore.VerificationStatus
import typealias BsvHomeCore.Wallet
import enum BsvHomeCore.WalletType
import Primitives

public final class BsvHomeAssetDetailsServiceMock: BsvHomeAssetDetailsServiceProtocol, @unchecked Sendable {
    private let assetPair: BsvHomeSwapPairSuggestion?
    private let headerActions: BsvHomeHeaderActions?
    private let showsPriceChange: Bool
    public var refreshFailures: [BsvHomeAssetRefreshFailure]
    private let refreshHandler: (@Sendable () async -> [BsvHomeAssetRefreshFailure])?
    public var bsvSnapshot: BsvHomeBsvAddressSnapshot = .mock()
    public var prepareBsvHandler: (@Sendable () async throws -> BsvHomeBsvAddressSnapshot)?
    public var syncBsvHandler: (@Sendable () async throws -> BsvHomeBsvAddressSnapshot)?
    public var continueBsvHandler: (@Sendable () async throws -> BsvHomeBsvAddressSnapshot)?
    public var addBsvHandler: (@Sendable (String) async throws -> BsvHomeBsvAddressSnapshot)?
    public var selectBsvHandler: (@Sendable (String, UInt32) async throws -> BsvHomeBsvAddressSnapshot)?
    public private(set) var addedBsvWalletIds: [String] = []
    public private(set) var selectedBsvAddresses: [(walletId: String, index: UInt32)] = []
    public var bsvGeneration: UInt64 = 0
    public var cancelledBsvWalletIds: [String] = []
    public private(set) var refreshCalls = 0

    public init(assetPair: BsvHomeSwapPairSuggestion? = nil, headerActions: BsvHomeHeaderActions? = nil, showsPriceChange: Bool = true, refreshFailures: [BsvHomeAssetRefreshFailure] = [], refreshHandler: (@Sendable () async -> [BsvHomeAssetRefreshFailure])? = nil) {
        self.assetPair = assetPair
        self.headerActions = headerActions
        self.showsPriceChange = showsPriceChange
        self.refreshFailures = refreshFailures
        self.refreshHandler = refreshHandler
    }

    public func refresh(assetId _: AssetId) async -> [BsvHomeAssetRefreshFailure] {
        refreshCalls += 1
        if let refreshHandler {
            return await refreshHandler()
        }
        return refreshFailures
    }

    public func prepareBsvAddresses(walletId _: String) async throws -> BsvHomeBsvAddressSnapshot {
        if let prepareBsvHandler {
            return try await prepareBsvHandler()
        }
        return bsvSnapshot
    }

    public func bsvAddresses(walletId _: String) async throws -> BsvHomeBsvAddressSnapshot? {
        bsvSnapshot
    }

    public func addBsvAddress(walletId: String) async throws -> BsvHomeBsvAddressSnapshot {
        addedBsvWalletIds.append(walletId)
        return try await addBsvHandler?(walletId) ?? bsvSnapshot
    }

    public func selectBsvAddress(walletId: String, index: UInt32) async throws -> BsvHomeBsvAddressSnapshot {
        selectedBsvAddresses.append((walletId, index))
        return try await selectBsvHandler?(walletId, index) ?? bsvSnapshot
    }

    public func bsvScanGeneration(walletId _: String) throws -> UInt64 {
        bsvGeneration
    }

    public func cancelBsvScan(walletId: String) throws {
        cancelledBsvWalletIds.append(walletId)
        bsvGeneration &+= 1
    }

    public func syncBsvAddresses(walletId _: String, generation: UInt64) async throws -> BsvHomeBsvAddressSnapshot {
        guard generation == bsvGeneration else { throw CancellationError() }
        if let syncBsvHandler {
            let snapshot = try await syncBsvHandler()
            guard generation == bsvGeneration else { throw CancellationError() }
            return snapshot
        }
        return bsvSnapshot
    }

    public func continueBsvScan(walletId _: String, generation: UInt64) async throws -> BsvHomeBsvAddressSnapshot {
        guard generation == bsvGeneration else { throw CancellationError() }
        if let continueBsvHandler {
            let snapshot = try await continueBsvHandler()
            guard generation == bsvGeneration else { throw CancellationError() }
            return snapshot
        }
        return bsvSnapshot
    }

    public func syncTransactions(assetId _: AssetId?) async throws {}

    public func updateBalances(assetIds _: [AssetId]) async throws {}

    public func setAssetPinned(assetId _: AssetId, pinned _: Bool) async throws {}

    public func setAssetsEnabled(assetIds _: [AssetId], enabled _: Bool) async throws {}

    public func addPrices(assetIds _: [AssetId]) async throws {}

    public func bannerContent(event _: BannerEvent, asset _: Asset?) -> BsvHomeBannerContent {
        BsvHomeBannerContent(icon: .none, title: .none, description: .none, link: .none)
    }

    public func applyBannerAction(key _: BsvHomeBannerKey, action _: BsvHomeBannerAction) async throws {}

    public func networkDestination(assetId _: AssetId) -> BsvHomeAssetNetworkDestination? {
        .none
    }

    public func verificationStatus(asset _: Asset, rank _: Int32) -> BsvHomeCore.VerificationStatus? {
        .none
    }

    public func state(wallet: Wallet, chain: Chain, metadata: AssetMetaData, balance _: BsvHomeAssetBalance, bannerEvents: [BannerEvent], price _: Double?, priceAlerts _: [BsvHomeCore.PriceAlert]) -> BsvHomeAssetDetailsState {
        let isWatchOnly = wallet.walletType == .view && !(chain == Primitives.Chain.bitcoinSv.rawValue && bsvColdWalletCanSend(wallet: wallet))
        let actions: BsvHomeHeaderActions = if wallet.walletType == .view, chain == Primitives.Chain.bitcoinSv.rawValue, BsvHomeRuntimeEnvironment.current == .wowPay {
            .buttons(buttons: [BsvHomeHeaderButton(kind: .receive, isEnabled: !bannerEvents.contains(.accountBlockedMultiSignature) && !bannerEvents.contains(.activateAsset))])
        } else {
            isWatchOnly ? .watchOnly : headerActions ?? .buttons(buttons: [])
        }
        return BsvHomeAssetDetailsState(
            isViewOnly: wallet.walletType == .view,
            headerActions: actions,
            showsBanners: wallet.walletType != .view,
            showsManage: !metadata.isBalanceEnabled,
            showsResources: false,
            showsPriceAlerts: false,
            showsPriceChange: showsPriceChange,
            showsEarn: false,
            emptyTransactionsAction: nil,
        )
    }

    public func swapPair(assetId: AssetId, hasBalance _: Bool) -> BsvHomeSwapPairSuggestion {
        assetPair ?? BsvHomeSwapPairSuggestion(payAssetId: assetId, receiveAssetId: nil)
    }

    public func explorerName(chain _: Chain) -> String {
        "Explorer"
    }

    public func addressUrl(chain _: Chain, address: String) -> BsvHomeCore.BlockExplorerLink {
        BsvHomeCore.BlockExplorerLink(name: "Explorer", link: "https://gemwallet.com/\(address)")
    }

    public func tokenUrl(chain _: Chain, address _: String) -> BsvHomeCore.BlockExplorerLink? {
        .none
    }

    public func setPriceAlert(assetId _: AssetId, enabled _: Bool) async throws {}

    public func syncPriceAlerts(assetId _: AssetId?) async throws {}

    public func deeplinkUrl(deeplink _: Deeplink) -> String {
        "https://gemwallet.com"
    }

    public func deeplinkBsvHomeUrl(deeplink _: Deeplink) -> String {
        "bsvhome://perpetuals"
    }
}
