// Copyright (c). Gem Wallet. All rights reserved.

import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices

public extension BsvHomeCoreWalletSessionStore {
    static func mock() -> BsvHomeCoreWalletSessionStore {
        BsvHomeCoreWalletSessionStore(store: BsvHomePreferencesStoreMock())
    }
}
