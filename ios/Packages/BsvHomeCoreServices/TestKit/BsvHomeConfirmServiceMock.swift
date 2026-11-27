// Copyright (c). Gem Wallet. All rights reserved.

public import typealias BsvHomeCore.AssetId
public import typealias BsvHomeCore.Chain
public import struct BsvHomeCore.BsvHomeAssetBalance
public import struct BsvHomeCore.BsvHomeConfirmData
public import enum BsvHomeCore.BsvHomeConfirmError
public import struct BsvHomeCore.BsvHomeConfirmInput
public import struct BsvHomeCore.BsvHomeConfirmLoadOptions
public import struct BsvHomeCore.BsvHomeConfirmMetadata
public import struct BsvHomeCore.BsvHomeConfirmPreload
public import protocol BsvHomeCore.BsvHomeConfirmServiceProtocol
public import struct BsvHomeCore.BsvHomeConfirmSimulation
public import enum BsvHomeCore.BsvHomeExecuteResult
public import struct BsvHomeCore.BsvHomeFeeAsset
public import enum BsvHomeCore.TransactionInputType
public import protocol BsvHomeCore.BsvHomeTransactionSigner
public import typealias BsvHomeCore.SimulationResult
public import typealias BsvHomeCore.Transaction
public import typealias BsvHomeCore.WalletId
import Foundation
import Primitives
public import struct BsvHomeCore.SimulationResult

public final class BsvHomeConfirmServiceMock: BsvHomeConfirmServiceProtocol, @unchecked Sendable {
    private let executeResult: Result<BsvHomeExecuteResult, any Error>
    private let metadataResult: Result<BsvHomeConfirmMetadata, any Error>
    private let feeAssetRows: [BsvHomeFeeAsset]
    private let preloadResult: Result<BsvHomeConfirmPreload, any Error>
    public let simulation: BsvHomeConfirmSimulation?
    private let lock = NSLock()
    private var inputs: [BsvHomeConfirmData] = []

    public var executedInputs: [BsvHomeConfirmData] { lock.withLock { inputs } }

    public init(
        execute: Result<BsvHomeExecuteResult, any Error> = .success(.sent(hashes: [], transactions: [])),
        metadata: Result<BsvHomeConfirmMetadata, any Error>? = nil,
        feeAssets: [BsvHomeFeeAsset] = [],
        preload: Result<BsvHomeConfirmPreload, any Error> = .failure(BsvHomeConfirmError.FeeRatesMissing),
        simulation: BsvHomeConfirmSimulation? = nil,
    ) {
        executeResult = execute
        metadataResult = metadata ?? .success(BsvHomeConfirmMetadata(
            assetBalance: BsvHomeAssetBalance(assetId: Primitives.Asset.mock().id.identifier, available: "0", frozen: "0", locked: "0", staked: "0", pending: "0", pendingUnconfirmed: "0", rewards: "0", reserved: "0", withdrawable: "0", earn: "0", metadata: nil, isActive: true),
            feeAssetBalance: BsvHomeAssetBalance(assetId: Primitives.Asset.mock().id.identifier, available: "0", frozen: "0", locked: "0", staked: "0", pending: "0", pendingUnconfirmed: "0", rewards: "0", reserved: "0", withdrawable: "0", earn: "0", metadata: nil, isActive: true),
            prices: [],
        ))
        feeAssetRows = feeAssets
        preloadResult = preload
        self.simulation = simulation
    }

    public func load(walletId _: WalletId, input _: BsvHomeConfirmInput, options _: BsvHomeConfirmLoadOptions) async throws -> BsvHomeConfirmData {
        try preloadResult.get().confirmData
    }

    public func metadata(walletId _: WalletId, assetId _: AssetId, feeAssetId _: AssetId, extraAssetIds _: [AssetId]) throws -> BsvHomeConfirmMetadata {
        try metadataResult.get()
    }

    public func preload(walletId _: WalletId, input _: BsvHomeConfirmInput, options _: BsvHomeConfirmLoadOptions) async throws -> BsvHomeConfirmPreload {
        try preloadResult.get()
    }

    public func feeAssets(walletId _: WalletId, chain _: Chain) async throws -> [BsvHomeFeeAsset] {
        feeAssetRows
    }

    public func syncMissingAssets(assetIds: [AssetId]) async throws -> [AssetId] {
        assetIds
    }

    public func trackPending() async throws {}

    public func track(walletId _: WalletId, transactions _: [Transaction]) async throws {}

    public func execute(confirm: BsvHomeConfirmData, signer _: any BsvHomeTransactionSigner) async throws -> BsvHomeExecuteResult {
        lock.withLock { inputs.append(confirm) }
        return try executeResult.get()
    }
}
