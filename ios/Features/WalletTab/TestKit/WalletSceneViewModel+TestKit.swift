// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeNftService
import protocol BsvHomeCore.BsvHomeWalletHomeServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import class BsvHomeCoreServices.ObservablePreferences
import BsvHomeCoreServicesTestKit
import NFT
import Primitives
import PrimitivesTestKit
import WalletTab

public extension WalletSceneViewModel {
    static func mock(wallet: Wallet = .mock(), observablePreferences: ObservablePreferences = .mock(), developmentAssetIds: [AssetId] = [], availableChains: [Chain] = Chain.productionCases, service: (any BsvHomeWalletHomeServiceProtocol)? = nil) -> WalletSceneViewModel {
        let mock = BsvHomeWalletHomeServiceMock()
        mock.developmentIds = developmentAssetIds.map(\.identifier)
        mock.availableChainIds = availableChains.map(\.rawValue)
        return WalletSceneViewModel(
            service: service ?? mock,
            observablePreferences: observablePreferences,
            collectionsModel: CollectionsViewModel(service: BsvHomeNftService.mock(), wallet: wallet),
            wallet: wallet,
            isPresentingSelectedAssetInput: .constant(.none),
            isPresentingWallets: .constant(false),
        )
    }
}
