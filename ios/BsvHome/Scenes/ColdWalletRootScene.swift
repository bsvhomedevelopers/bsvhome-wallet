import BsvColdWallet
import Components
import Foundation
import BsvHomeCore
import BsvHomeCoreServices
import Keychain
import Localization
import Store
import Style
import SwiftUI
import UIKit

struct ColdWalletRootScene: View {
    @Environment(\.scenePhase) private var scenePhase
    @State private var context: BsvColdWalletContext?
    @State private var navigation = NavigationPathState()
    @State private var failed = false
    private let keychain = KeychainDefault(service: "com.bsvhome.ios.cold-wallet")

    var body: some View {
        content
            .tint(Colors.WowPay.brand)
            .task(id: failed) {
                guard context == nil, !failed else { return }
                guard UIApplication.shared.isProtectedDataAvailable else {
                    failed = true
                    return
                }
                do { context = try await makeContext() }
                catch { failed = true }
            }
            .onChange(of: scenePhase) { _, phase in
                if phase == .background {
                    context?.cancelActions()
                }
            }
            .onDisappear { context?.cancel() }
    }

    @ViewBuilder
    private var content: some View {
        if let context {
            NavigationStack(path: navigation.binding) {
                BsvColdWalletScene(model: context.model, navigation: navigation)
                    .id(ObjectIdentifier(context))
                    .bsvColdWalletNavigation(model: context.model, navigation: navigation)
                    .bsvColdWalletPassword(authorization: context.authorization)
            }
        } else if failed {
            ContentUnavailableView {
                Label(Localized.coldWalletTitle, systemImage: "exclamationmark.triangle")
            } description: {
                Text(Localized.coldWalletStartupStorageError)
            } actions: {
                Button(Localized.Common.tryAgain) { failed = false }
            }
        } else {
            ProgressView()
        }
    }

    private func makeContext() async throws -> BsvColdWalletContext {
        let authorization = BsvColdWalletPasswordAuthorization(store: BsvHomeCoreSecurePreferencesStore(namespace: "offline_signing_password_", keychain: keychain, accessibility: .whenUnlockedThisDeviceOnly))
        let dependencies = try await Task.detached(priority: .userInitiated) { [keychain] in
            let directory = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true).appendingPathComponent("ColdWallet", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.protectionKey: FileProtectionType.complete])
            try WalletFileProtection.apply(to: directory)

            let db = try DB(fileURL: directory.appendingPathComponent("offline.sqlite"))
            let keystorePassword = LocalKeystorePassword(keychain: keychain)
            let coldStore = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("Transactions", isDirectory: true))
            let localKeystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("Keystore", isDirectory: true), keystorePassword: keystorePassword, environment: .wowPay, coldWalletStore: coldStore)
            let password = BsvHomeCoreKeystorePassword(keystore: localKeystore)
            let namespace = "cold_wallet_"
            let preferences = BsvHomeCorePreferencesStore(namespace: namespace)
            let appPreferences = BsvHomePreferencesService(store: preferences)
            let walletStore = BsvHomeCoreWalletStore(store: WalletStore(db: db))
            let session = BsvHomeWalletSessionService.newWithEnvironment(store: BsvHomeCoreWalletSessionStore(store: preferences), wallets: walletStore, environment: .wowPay)
            let gateway = BsvHomeGateway.newWithEnvironment(provider: OfflineProvider(), preferences: preferences, securePreferences: BsvHomeCoreSecurePreferencesStore(namespace: "cold_wallet_gateway", keychain: keychain, accessibility: .whenUnlockedThisDeviceOnly), environment: .wowPay)
            let addresses = BsvHomeBsvAddressPool(gateway: gateway, walletStore: walletStore, store: BsvHomeCoreBsvAddressStore(store: BsvAddressPoolStore(db: db)), keystore: localKeystore.bsvHomeKeystore, password: password)
            let walletService = BsvHomeWalletService(
                keystore: localKeystore.bsvHomeKeystore,
                password: password,
                store: walletStore,
                session: session,
                appPreferences: appPreferences,
                files: BsvHomeCoreFileStore(),
                preferences: BsvHomeWalletPreferencesService(store: BsvHomeCoreWalletPreferencesStore(namespace: namespace)),
                explorer: BsvHomeExplorerService(preferences: appPreferences),
                addresses: BsvHomeCoreAddressStore(store: AddressStore(db: db)),
                localizer: BsvHomeCoreLocalizer(),
                bsvAddresses: addresses,
            )
            let signer = BsvHomeCoreBsvColdWalletSigner(keystore: localKeystore, authenticator: BiometryAuthenticationService(keystorePassword: keystorePassword, securityService: BsvHomeSecurityService()), reason: Localized.Settings.Security.authentication, authorizeSigning: { wallet, scope in try await authorization.authorize(wallet: wallet, scope: scope) })
            let service = BsvHomeBsvColdWalletService.offline(wallet: walletService, addresses: addresses, keystore: localKeystore.bsvHomeKeystore, store: coldStore, signer: signer)
            try WalletFileProtection.apply(to: directory)
            return (service: service, directory: directory)
        }.value
        let model = BsvColdWalletViewModel(service: dependencies.service, mode: .offline, protectStorage: {
            try await Task.detached(priority: .userInitiated) {
                try WalletFileProtection.apply(to: dependencies.directory)
            }.value
        }, cancelAuthorization: authorization.cancel(scopeId:))
        return BsvColdWalletContext(model: model, authorization: authorization)
    }
}
