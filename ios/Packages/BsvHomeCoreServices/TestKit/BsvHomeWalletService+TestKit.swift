// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeExplorerService
import enum BsvHomeCore.BsvHomeLocalizedText
import protocol BsvHomeCore.BsvHomeLocalizer
import class BsvHomeCore.BsvHomeNameService
import class BsvHomeCore.BsvHomePaymentService
import class BsvHomeCore.BsvHomePreferencesService
import class BsvHomeCore.BsvHomeRecipientService
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import class BsvHomeCore.BsvHomeSignMessageService
import class BsvHomeCore.BsvHomeWalletPreferencesService
import class BsvHomeCore.BsvHomeWalletService
import class BsvHomeCore.BsvHomeWalletSessionService
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import Primitives
import Store
import StoreTestKit

public extension BsvHomeWalletService {
    static func mock(
        keystore: LocalKeystore = LocalKeystore.mock(),
        db: DB = .mock(),
        sessionStore: BsvHomeCoreWalletSessionStore = .mock(),
        environment: BsvHomeRuntimeEnvironment = .production,
    ) -> BsvHomeWalletService {
        let bsvHomeWalletStore = BsvHomeCoreWalletStore(store: WalletStore(db: db))
        let appPreferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        return BsvHomeWalletService(
            keystore: keystore.bsvHomeKeystore,
            password: BsvHomeCoreKeystorePassword(keystore: keystore),
            store: bsvHomeWalletStore,
            session: BsvHomeWalletSessionService.newWithEnvironment(store: sessionStore, wallets: bsvHomeWalletStore, environment: environment),
            appPreferences: appPreferences,
            files: BsvHomeCoreFileStore(),
            preferences: BsvHomeWalletPreferencesService.mock(),
            explorer: BsvHomeExplorerService(preferences: appPreferences),
            addresses: BsvHomeCoreAddressStore(store: AddressStore(db: db)),
            localizer: TestLocalizer(),
            bsvAddresses: GatewayService.mock().bsvAddressPool(
                walletStore: bsvHomeWalletStore,
                store: BsvHomeCoreBsvAddressStore(store: BsvAddressPoolStore(db: db)),
                keystore: keystore.bsvHomeKeystore,
                password: BsvHomeCoreKeystorePassword(keystore: keystore),
            ),
        )
    }
}

public extension BsvHomeRecipientService {
    static func mock() -> BsvHomeRecipientService {
        BsvHomeRecipientService(names: BsvHomeNameService.mock(), payments: BsvHomePaymentService.mock(), session: BsvHomeWalletSessionService.mock())
    }
}

public extension BsvHomeWalletSessionService {
    static func mock(
        store: WalletStore = .mock(),
        sessionStore: BsvHomeCoreWalletSessionStore = .mock(),
    ) -> BsvHomeWalletSessionService {
        BsvHomeWalletSessionService(store: sessionStore, wallets: BsvHomeCoreWalletStore(store: store))
    }

    static func mock(wallet: Wallet) throws -> BsvHomeWalletSessionService {
        let store = WalletStore.mock(db: .mock())
        try store.addWallet(wallet)
        return BsvHomeWalletSessionService(store: BsvHomeCoreWalletSessionStore.mock(), wallets: BsvHomeCoreWalletStore(store: store))
    }
}

public extension BsvHomeSignMessageService {
    static func mock(keystore: LocalKeystore = LocalKeystore.mock()) -> BsvHomeSignMessageService {
        BsvHomeSignMessageService(
            names: BsvHomeNameService.mock(),
            explorer: BsvHomeExplorerService(preferences: BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())),
            keystore: keystore.bsvHomeKeystore,
            password: BsvHomeCoreKeystorePassword(keystore: keystore),
        )
    }
}

final class TestLocalizer: BsvHomeLocalizer, Sendable {
    func text(text: BsvHomeLocalizedText) -> String {
        switch text {
        case let .walletDefaultName(index): "Wallet #\(index)"
        case let .walletDefaultNameChain(chain, index): "\(chain) Wallet #\(index)"
        }
    }
}
