// Copyright (c). Gem Wallet. All rights reserved.

import class BsvHomeCore.BsvHomeRecentActivityService
import BsvHomeCoreServicesTestKit
import Foundation
import Primitives
import Recents
import StoreTestKit

public extension RecentsSceneViewModel {
    static func mock(
        walletId: WalletId = .mock(),
        types: [RecentActivityType] = [],
        onSelect: @escaping (Asset) -> Void = { _ in },
    ) -> RecentsSceneViewModel {
        RecentsSceneViewModel(
            walletId: walletId,
            types: types,
            service: BsvHomeRecentActivityService(store: BsvHomeCoreRecentActivityStore(store: .mock()), session: .mock()),
            onSelect: onSelect,
        )
    }
}
