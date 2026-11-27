// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomePreferencesService
import class BsvHomeCore.BsvHomePriceService
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit

public extension BsvHomePriceService {
    static func mock(db: DB = .mock(), preferences: BsvHomePreferencesService = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())) -> BsvHomePriceService {
        BsvHomePriceService(
            store: BsvHomeCorePriceStore(priceStore: .mock(db: db), fiatRateStore: .mock(db: db)),
            preferences: preferences,
        )
    }
}
