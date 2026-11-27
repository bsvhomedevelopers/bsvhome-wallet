// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeDeviceApiClient
import class BsvHomeCore.BsvHomeDeviceKeyService
import class BsvHomeCore.BsvHomeAvatarService
import class BsvHomeCore.BsvHomeCollectibleService
import class BsvHomeCore.BsvHomeExplorerService
import class BsvHomeCore.BsvHomeNftService
import protocol BsvHomeCore.BsvHomeNftServiceProtocol
import NativeProviderService
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit

public extension BsvHomeCollectibleService {
    static func mock(nftStore: NftStore = .mock(), explorer: BsvHomeExplorerService = .mock()) -> BsvHomeCollectibleService {
        BsvHomeCollectibleService(
            nfts: BsvHomeNftService.mock(nftStore: nftStore),
            avatars: BsvHomeAvatarService(wallets: BsvHomeCoreWalletStore(store: .mock()), files: BsvHomeCoreFileStore(), provider: NativeProvider()),
            explorer: explorer,
        )
    }
}

public extension BsvHomeNftService {
    static func mock(nftStore: NftStore = .mock()) -> BsvHomeNftService {
        BsvHomeNftService(
            api: BsvHomeDeviceApiClient(
                provider: NativeProvider(),
                deviceKey: BsvHomeDeviceKeyService(store: BsvHomeSecureStoreMock()),
            ),
            store: BsvHomeCoreNftStore(store: nftStore),
            session: .mock(),
        )
    }
}
