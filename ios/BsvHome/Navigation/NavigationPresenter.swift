// Copyright (c). Gem Wallet. All rights reserved.

import Components
import protocol BsvHomeCore.BsvHomeAssetsServiceProtocol
import protocol BsvHomeCore.BsvHomeNftServiceProtocol
import protocol BsvHomeCore.BsvHomeRecentActivityServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import NFT
import Primitives
import PrimitivesComponents
import SwiftUI
import Transactions

@Observable
final class NavigationPresenter: Sendable {
    @MainActor private var _isPresentingAssetInput: SelectedAssetInput?
    @MainActor private var _isPresentingPayment: PaymentDestination?
    @MainActor private var _isPresentingPriceAlert: Asset?
    @MainActor private(set) var multisigRequest: WowPayMultisigNavigationRequest?
    @MainActor private var _isPresentingSupport: Bool = false
    @MainActor private var _isPresentingWallets: Bool = false
    private let assetsService: any BsvHomeAssetsServiceProtocol
    private let nftService: any BsvHomeNftServiceProtocol
    private let recentActivity: any BsvHomeRecentActivityServiceProtocol

    init(
        assetsService: any BsvHomeAssetsServiceProtocol,
        nftService: any BsvHomeNftServiceProtocol,
        recentActivity: any BsvHomeRecentActivityServiceProtocol,
    ) {
        self.assetsService = assetsService
        self.nftService = nftService
        self.recentActivity = recentActivity
    }
}

@MainActor
extension NavigationPresenter {
    var isPresentingAssetInput: Binding<SelectedAssetInput?> {
        Binding(get: { self._isPresentingAssetInput }, set: { input in
            guard input?.type.isAvailableForPresentation != false else { return }
            self._isPresentingAssetInput = input
        })
    }

    var isPresentingPayment: Binding<PaymentDestination?> {
        Binding(get: { self._isPresentingPayment }, set: { self._isPresentingPayment = $0 })
    }

    var isPresentingPriceAlert: Binding<Asset?> {
        Binding(get: { self._isPresentingPriceAlert }, set: { self._isPresentingPriceAlert = $0 })
    }

    var isPresentingSupport: Binding<Bool> {
        Binding(get: { self._isPresentingSupport }, set: { self._isPresentingSupport = $0 })
    }

    var isPresentingWallets: Binding<Bool> {
        Binding(get: { self._isPresentingWallets }, set: { self._isPresentingWallets = $0 })
    }

    func requestMultisig(_ destination: WowPayMultisigNavigationRequest.Destination, navigationMode: WowPayMultisigNavigationRequest.NavigationMode = .replaceStack, isActive: @escaping @MainActor () -> Bool = { true }) {
        let request = WowPayMultisigNavigationRequest(destination: destination, navigationMode: navigationMode, isActive: isActive)
        guard isActive() else {
            request.cancel()
            return
        }
        multisigRequest?.cancel()
        multisigRequest = request
    }

    func takeMultisigRequest(id: UUID) -> WowPayMultisigNavigationRequest? {
        guard let request = multisigRequest, request.id == id else { return nil }
        multisigRequest = nil
        guard request.isActive() else {
            request.cancel()
            return nil
        }
        return request
    }

    func cancelMultisigRequest() {
        multisigRequest?.cancel()
        multisigRequest = nil
    }

    func presentAssetInput(type: SelectedAssetType, for asset: Asset, wallet: Wallet) throws {
        let account = try wallet.account(for: asset.chain)
        isPresentingAssetInput.wrappedValue = SelectedAssetInput(
            type: type,
            assetData: .with(asset: asset, account: account),
        )
    }

    func presentSwap(
        from fromAssetId: AssetId,
        to toAssetId: AssetId?,
        wallet: Wallet,
    ) async throws {
        let fromAsset = try await assetsService.ensureAsset(for: fromAssetId)
        let toAsset: Asset? = if let toAssetId {
            try await assetsService.ensureAsset(for: toAssetId)
        } else {
            nil
        }
        try presentAssetInput(type: .swap(fromAsset, toAsset), for: fromAsset, wallet: wallet)
    }

    func handleTransactionHeaderAction(
        _ action: TransactionHeaderAction,
        wallet: Wallet,
        navigationState: NavigationStateManager,
        nftDestination: NavigationPathState,
    ) async throws {
        switch action {
        case let .asset(assetId), let .perpetual(assetId):
            guard let asset = try await assetsService.openAsset(for: assetId) else {
                return
            }
            navigationState.openAsset(asset)
        case let .swap(fromAssetId, toAssetId):
            try await presentSwap(
                from: fromAssetId,
                to: toAssetId,
                wallet: wallet,
            )
        case let .nft(assetId):
            let assetData = try await nftService.ensureAsset(assetId: assetId.identifier).map()
            nftDestination.append(Scenes.Collectible(assetData: assetData))
        }
    }

    func recordRecent(input: SelectedAssetInput) {
        guard let action = input.type.action else { return }
        Task { try? await recentActivity.addRecent(action: action, asset: input.asset.map()) }
    }

    func completeSwap(fromAsset: Asset, navigationState: NavigationStateManager) async throws {
        let asset = try await assetsService.ensureAsset(for: fromAsset.id)
        switch navigationState.selectedTab {
        case .wallet, .home, .swap:
            navigationState.openWallet(path: [Scenes.Asset(asset: asset)])
        case .settings:
            break
        }
        isPresentingAssetInput.wrappedValue = nil
    }
}
