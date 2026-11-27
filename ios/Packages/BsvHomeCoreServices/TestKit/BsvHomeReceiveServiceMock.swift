import typealias BsvHomeCore.Asset
import typealias BsvHomeCore.AssetId
import typealias BsvHomeCore.Chain
import struct BsvHomeCore.BsvHomeBsvAddressSnapshot
import enum BsvHomeCore.BsvHomeMemoWarning
import protocol BsvHomeCore.BsvHomeReceiveServiceProtocol
import struct BsvHomeCore.Wallet
import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit

public final class BsvHomeReceiveServiceMock: BsvHomeReceiveServiceProtocol, @unchecked Sendable {
    public var snapshot: BsvHomeBsvAddressSnapshot = .mock()
    public var prepareHandler: (@Sendable () async throws -> BsvHomeBsvAddressSnapshot)?
    public var syncHandler: (@Sendable () async throws -> BsvHomeBsvAddressSnapshot)?
    public var assetHandler: (@Sendable (AssetId) async throws -> Asset)?
    public var addHandler: (@Sendable (String) async throws -> BsvHomeBsvAddressSnapshot)?
    public var selectHandler: (@Sendable (String, UInt32) async throws -> BsvHomeBsvAddressSnapshot)?
    public private(set) var prepareCalls = 0
    public private(set) var continueCalls = 0
    public var bsvGeneration: UInt64 = 0
    public var cancelledBsvWalletIds: [String] = []

    public init() {}

    public func prepareBsvAddresses(walletId _: String) async throws -> BsvHomeBsvAddressSnapshot {
        prepareCalls += 1
        if let prepareHandler {
            return try await prepareHandler()
        }
        return snapshot
    }

    public func bsvAddresses(walletId _: String) async throws -> BsvHomeBsvAddressSnapshot? {
        snapshot
    }

    public func addBsvAddress(walletId: String) async throws -> BsvHomeBsvAddressSnapshot {
        try await addHandler?(walletId) ?? snapshot
    }

    public func selectBsvAddress(walletId: String, index: UInt32) async throws -> BsvHomeBsvAddressSnapshot {
        try await selectHandler?(walletId, index) ?? snapshot
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
        if let syncHandler {
            let snapshot = try await syncHandler()
            guard generation == bsvGeneration else { throw CancellationError() }
            return snapshot
        }
        return snapshot
    }

    public func continueBsvScan(walletId _: String, generation: UInt64) async throws -> BsvHomeBsvAddressSnapshot {
        continueCalls += 1
        guard generation == bsvGeneration else { throw CancellationError() }
        return snapshot
    }

    public func memoWarning(chain _: Chain) -> BsvHomeMemoWarning {
        .notSupported
    }

    public func networkAssetIds(assetId: AssetId, associations _: [AssetId], wallet _: Wallet) -> [AssetId] {
        [assetId]
    }

    public func enableAsset(walletId _: String, assetId _: AssetId) async throws {}

    public func syncMissingAssets(assetIds _: [AssetId]) async throws -> [AssetId] {
        []
    }

    public func asset(assetId: AssetId) async throws -> Asset {
        if let assetHandler {
            return try await assetHandler(assetId)
        }
        return Primitives.Asset.mock(id: Primitives.AssetId(core: assetId)).map()
    }
}

import Foundation
