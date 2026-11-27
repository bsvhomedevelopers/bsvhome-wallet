// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomePreferencesStore
import enum BsvHomeCore.BsvHomeServiceError
import class BsvHomeCore.BsvHomeWalletService
import class BsvHomeCore.BsvHomeWalletSessionService
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Localization
@testable import ManageWallets
import Primitives
import PrimitivesTestKit
@testable import Store
import StoreTestKit
import SwiftUI
import Testing

@MainActor
struct WalletsSceneViewModelTests {
    @Test
    func visibleWalletsPreserveStoredAccounts() {
        let legacy = Wallet.mock(id: .single(chain: .ethereum, address: "legacy"), type: .single, accounts: [.mock(chain: .ethereum, address: "legacy")])
        let mixed = Wallet.mock(accounts: [.mock(chain: .ethereum), .mock(chain: .tronShasta), .mock(chain: .bitcoinSv), .mock(chain: .tron)])
        let model = WalletsSceneViewModel.mock()
        model.walletsQuery.value = [legacy, mixed]
        model.pinnedWalletsQuery.value = [legacy, mixed]

        #expect(model.wallets.map(\.id) == [mixed.id])
        #expect(model.wallets.first?.accounts.map(\.chain) == [.bitcoinSv])
        #expect(model.pinnedWallets.map(\.id) == [mixed.id])
        #expect(model.walletsQuery.value == [legacy, mixed])
        #expect(model.pinnedWalletsQuery.value == [legacy, mixed])
    }

    @Test(arguments: [false, true])
    func walletSelectionCompletesOnlyAfterSessionPersistence(failsWrite: Bool) throws {
        let db = DB.mock()
        let chains: [Chain] = [.bitcoinSv, .ethereum, .tronShasta, .tron]
        try AssetStore.mock(db: db).add(assets: chains.map { .mock(asset: .mock(id: $0.assetId)) })
        let walletStore = WalletStore.mock(db: db)
        let current = Wallet.mock(id: .multicoin(address: "current"), accounts: [.mock(chain: .bitcoinSv, address: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5")])
        let target = Wallet.mock(
            id: .multicoin(address: "target"),
            accounts: [.mock(chain: .ethereum), .mock(chain: .bitcoinSv, address: "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD"), .mock(chain: .tronShasta), .mock(chain: .tron)],
        )
        try walletStore.addWallet(current)
        try walletStore.addWallet(target)
        let originalWallets = try walletStore.getWallets()
        let preferences = WalletSelectionPreferences()
        let sessionStore = BsvHomeCoreWalletSessionStore(store: preferences)
        try sessionStore.setCurrentWalletId(walletId: current.id.id)
        let service = BsvHomeWalletService.mock(db: db, sessionStore: sessionStore, environment: .wowPay)
        let model = WalletsSceneViewModel.mock(walletService: service)
        model.walletsQuery.value = originalWallets
        let selection = try #require(model.wallets.first { $0.id == target.id })
        #expect(selection.accounts.map(\.chain) == [.bitcoinSv])
        preferences.failWrites(failsWrite)
        var selectedWallets: [WalletId?] = []

        model.onSelect(wallet: selection) { selectedWallets.append(model.currentWalletId) }

        #expect(model.currentWalletId == (failsWrite ? current.id : target.id))
        #expect(selectedWallets == (failsWrite ? [] : [target.id]))
        #expect((model.isPresentingAlertMessage != nil) == failsWrite)
        #expect(try walletStore.getWallets() == originalWallets)
        #expect(model.walletsQuery.value == originalWallets)

        if failsWrite {
            preferences.failWrites(false)
            model.onSelect(wallet: selection) { selectedWallets.append(model.currentWalletId) }
            #expect(model.currentWalletId == target.id)
            #expect(selectedWallets == [target.id])
            #expect(model.isPresentingAlertMessage == nil)
            #expect(try walletStore.getWallets() == originalWallets)
        }
    }

    @Test
    func walletDetailsProjectOnlyAllowedAccounts() {
        let wallet = Wallet.mock(accounts: [.mock(chain: .ethereum), .mock(chain: .bitcoinSv), .mock(chain: .tronShasta), .mock(chain: .tron)])
        let model = WalletDetailViewModel(
            navigationPath: .constant(NavigationPath()),
            wallet: wallet,
            service: BsvHomeWalletService.mock(),
            preferences: .mock(),
        )

        #expect(model.wallet.accounts.map(\.chain) == [.bitcoinSv])
        #expect(model.walletQuery.value == wallet)
        let hidden = Wallet.mock(id: .single(chain: .ethereum, address: "legacy"), type: .single, accounts: [.mock(chain: .ethereum, address: "legacy")])
        model.walletQuery.value = hidden
        #expect(model.address == nil)
        #expect(model.walletQuery.value == hidden)
    }

    @Test
    func missingSecretMetadataKeepsWalletDetailsWithoutOfferingAnIncorrectBackup() async {
        let wallet = Wallet.mock(accounts: [.mock(chain: .bitcoinSv)])
        let model = WalletDetailViewModel(
            navigationPath: .constant(NavigationPath()),
            wallet: wallet,
            service: BsvHomeWalletService.mock(),
            preferences: .mock(),
        )
        await model.loadSecretKind()
        #expect(model.secretKind == nil)
        #expect(model.secretMetadataError == Localized.bsvHdBackupUnavailable)
        #expect(model.wallet.accounts == wallet.accounts)
        model.onShowSecret()
        #expect(model.isRevealingSecret == false)
        #expect(model.isPresentingExportWallet == nil)
    }

    @Test
    func onDeleteConfirmed() async throws {
        let db = DB.mock()
        let walletStore = WalletStore.mock(db: db)
        for address in ["0x1", "0x2", "0x3"] {
            try walletStore.addWallet(.mock(id: .multicoin(address: address)))
        }

        let sessionStore = BsvHomeCoreWalletSessionStore.mock()
        let session = BsvHomeWalletSessionService(store: sessionStore, wallets: BsvHomeCoreWalletStore(store: walletStore))
        let service = BsvHomeWalletService.mock(db: db, sessionStore: sessionStore)
        try session.setCurrent(walletId: .multicoin(address: "0x1"))

        let model = WalletsSceneViewModel.mock(walletService: service)
        model.walletsQuery.value = await session.wallets

        #expect(model.currentWalletId == .multicoin(address: "0x1"))

        await model.onDeleteConfirmed(wallet: .mock(id: .multicoin(address: "0x1")))

        #expect(model.currentWalletId == .multicoin(address: "0x2"))

        await model.onDeleteConfirmed(wallet: .mock(id: .multicoin(address: "0x2")))

        #expect(model.currentWalletId == .multicoin(address: "0x3"))

        await model.onDeleteConfirmed(wallet: .mock(id: .multicoin(address: "0x3")))

        #expect(model.currentWalletId == .none)
    }
}

private final class WalletSelectionPreferences: BsvHomePreferencesStore, @unchecked Sendable {
    private let lock = NSLock()
    private var values: [String: String] = [:]
    private var writesFail = false

    func failWrites(_ value: Bool) {
        lock.withLock { writesFail = value }
    }

    func get(key: String) -> String? {
        lock.withLock { values[key] }
    }

    func set(key: String, value: String) throws {
        try lock.withLock {
            if writesFail { throw BsvHomeServiceError.Store(msg: "wallet session write failed") }
            values[key] = value
        }
    }

    func remove(key: String) throws {
        _ = lock.withLock { values.removeValue(forKey: key) }
    }

    func clear() throws {
        lock.withLock { values.removeAll() }
    }
}

// MARK: - Mock Extensions

extension WalletsSceneViewModel {
    static func mock(
        navigationPath: Binding<NavigationPath> = .constant(NavigationPath()),
        walletService: BsvHomeWalletService = .mock(),
        isPresentingCreateWalletSheet: Binding<Bool> = .constant(false),
        isPresentingImportWalletSheet: Binding<Bool> = .constant(false),
    ) -> WalletsSceneViewModel {
        WalletsSceneViewModel(
            navigationPath: navigationPath,
            walletService: walletService,
            preferences: .mock(),
            isPresentingCreateWalletSheet: isPresentingCreateWalletSheet,
            isPresentingImportWalletSheet: isPresentingImportWalletSheet,
        )
    }
}
