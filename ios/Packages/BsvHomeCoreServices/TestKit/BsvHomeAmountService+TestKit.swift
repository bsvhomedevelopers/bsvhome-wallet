// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeAmountService
import class BsvHomeCore.BsvHomePreferencesService
import class BsvHomeCore.BsvHomeStaticApiClient
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import NativeProviderService
import StoreTestKit

public extension BsvHomeAmountService {
    static func mock() -> BsvHomeAmountService {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        return BsvHomeAmountService(
            stake: GatewayService.mock().stakeService(
                staticApi: BsvHomeStaticApiClient(provider: NativeProvider()),
                store: BsvHomeCoreStakeStore(store: .mock()),
                addressStore: BsvHomeCoreAddressStore(store: .mock()),
                explorer: .mock(),
                preferences: preferences,
                session: .mock(),
            ),
            preferences: preferences,
            session: .mock(),
        )
    }
}
