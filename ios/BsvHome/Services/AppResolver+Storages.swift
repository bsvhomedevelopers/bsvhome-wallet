// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Store

extension AppResolver {
    struct Storages {
        let db: DB = .init()
        let storeManager: StoreManager
        let keystore = LocalKeystore(environment: .current)

        init() {
            storeManager = StoreManager(db: db)
        }
    }
}
