// Copyright (c). Gem Wallet. All rights reserved.

public import struct BsvHomeCore.BlockExplorerLink
public import typealias BsvHomeCore.Chain
public import typealias BsvHomeCore.Currency
public import enum BsvHomeCore.BsvHomeAcquireAssetFlow
public import class BsvHomeCore.BsvHomeAssetConfigService
public import struct BsvHomeCore.BsvHomeAutocloseSummary
public import struct BsvHomeCore.BsvHomeConfirmLoad
public import struct BsvHomeCore.BsvHomeConfirmLoadOptions
public import struct BsvHomeCore.BsvHomeConfirmScreen
public import protocol BsvHomeCore.BsvHomeConfirmSessionProtocol
public import enum BsvHomeCore.BsvHomeExecuteResult
public import enum BsvHomeCore.BsvHomeKeystoreAuthentication
public import typealias BsvHomeCore.PerpetualModifyConfirmData
import Foundation
import BsvHomeCorePrimitivesTestKit
import Primitives

public final class BsvHomeConfirmSessionMock: BsvHomeConfirmSessionProtocol, @unchecked Sendable {
    private let initialState: BsvHomeConfirmLoad
    private let loadResult: Result<BsvHomeConfirmLoad, any Error>
    private let executeResult: Result<BsvHomeExecuteResult, any Error>
    private let authenticationValue: BsvHomeKeystoreAuthentication
    private let assetConfig = BsvHomeAssetConfigService()
    private var loaded: BsvHomeConfirmLoad?
    private let cancellationLock = NSLock()
    private var cancellationCount = 0
    public var onLoad: (@MainActor () -> Void)?
    public var onLoadAsync: (@MainActor () async throws -> BsvHomeConfirmLoad)?
    public var onExecute: (@MainActor () -> Void)?
    public var onExecuteAsync: (@MainActor () async throws -> BsvHomeExecuteResult)?
    public var onCancel: (@Sendable () -> Void)?

    public var cancelCount: Int { cancellationLock.withLock { cancellationCount } }

    public func cancel() {
        cancellationLock.withLock { cancellationCount += 1 }
        onCancel?()
    }

    public init(
        state: BsvHomeConfirmLoad = .mock(),
        load: Result<BsvHomeConfirmLoad, any Error> = .success(.mock()),
        execute: Result<BsvHomeExecuteResult, any Error> = .success(.signed(data: [])),
        authentication: BsvHomeKeystoreAuthentication = .none,
    ) {
        initialState = state
        loadResult = load
        executeResult = execute
        authenticationValue = authentication
    }

    public func screen() -> BsvHomeConfirmScreen {
        BsvHomeConfirmScreen(phase: .loading, amountFailed: false, hasCriticalWarning: false, failure: nil)
    }

    @MainActor
    public func state() async throws -> BsvHomeConfirmLoad {
        loaded ?? initialState
    }

    @MainActor
    public func load(options _: BsvHomeConfirmLoadOptions) async throws -> BsvHomeConfirmLoad {
        onLoad?()
        let result = try await onLoadAsync?() ?? loadResult.get()
        loaded = result
        return result
    }

    @MainActor
    public func execute() async throws -> BsvHomeExecuteResult {
        onExecute?()
        return try await onExecuteAsync?() ?? executeResult.get()
    }

    public func getCurrency() -> Currency {
        Primitives.Currency.usd.rawValue
    }

    public func authentication() -> BsvHomeKeystoreAuthentication {
        authenticationValue
    }

    public func addressUrl(chain: Chain, address: String) -> BlockExplorerLink {
        BlockExplorerLink(name: "Explorer", link: "https://explorer.test/\(chain)/\(address)")
    }

    public func acquireAssetFlow(chain: Chain) -> BsvHomeAcquireAssetFlow {
        assetConfig.acquireFlow(chain: chain)
    }

    public func insufficientNetworkFeeBuyAmount() -> Int32 {
        Self.networkFeeBuyAmount
    }

    public func autocloseSummary(data _: PerpetualModifyConfirmData) -> BsvHomeAutocloseSummary? {
        nil
    }

    public static let networkFeeBuyAmount: Int32 = 10
}
