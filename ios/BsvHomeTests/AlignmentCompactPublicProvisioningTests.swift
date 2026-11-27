import Foundation
@testable import BsvHome
import BsvHomeCore
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import GRDB
import Primitives
import Store
import Testing

@MainActor
struct AlignmentCompactPublicProvisioningTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COMPACT_PUBLIC_QA"] == "1", "Requires explicit public-only provisioning of the exact Phase8 compact base App."))
    func importOnePublicAddressOrVerifyItsExistingMarker() async {
        do {
            let support = try validateHost()
            let publicDirectory = support.appendingPathComponent("Phase8CompactPublicQA", isDirectory: true)
            let markerURL = publicDirectory.appendingPathComponent("public-marker.json")
            let marker = try readMarker(markerURL)
            try validatePublicDirectory(publicDirectory, marker: marker)
            guard try missingOrEmpty(support.appendingPathComponent("ColdWallet", isDirectory: true)),
                  try missingOrEmpty(support.appendingPathComponent("WowPayColdWallet", isDirectory: true))
            else { throw Rejection.invalidState }
            let purpose = try DevicePurposeStore.appGroup.read()
            guard purpose == nil || purpose == .daily else { throw Rejection.invalidState }
            let context = try makeContext(support)
            let before = try context.wallets.getWallets()
            guard try !context.keystore.hasStoredWallets() else { throw Rejection.invalidState }
            if let marker {
                guard marker == expectedMarker(), before.count == 1,
                      try context.service.currentWalletId() == marker.walletId,
                      purpose == .daily
                else { throw Rejection.invalidState }
                try await verify(before[0], context: context)
                #expect(context.ports.calls == 0)
                let publicWalletsUnchanged = try context.wallets.getWallets() == before
                #expect(publicWalletsUnchanged)
                return
            }
            guard before.isEmpty, context.preferences.get(key: "current_wallet_id") == nil,
                  !Task.isCancelled
            else { throw Rejection.invalidState }
            try await seedCanonicalAsset(context)
            let result = try await context.service.importWallet(
                name: Self.walletName,
                import: .address(address: Self.address, chain: Primitives.Chain.bitcoinSv.rawValue),
                source: .import,
            )
            guard case let .new(wallet) = result, !Task.isCancelled else { throw Rejection.invalidState }
            let native = wallet.map()
            try await verify(native, context: context)
            let after = try context.wallets.getWallets()
            guard after.count == 1, after[0] == native, !Task.isCancelled else { throw Rejection.invalidState }
            guard try DevicePurposeStore.appGroup.confirm(.daily) == .daily else { throw Rejection.invalidState }
            try context.service.setCurrentWalletId(walletId: native.id.id)
            guard try context.service.currentWalletId() == native.id.id,
                  context.ports.calls == 0, !Task.isCancelled
            else { throw Rejection.invalidState }
            try FileManager.default.createDirectory(at: publicDirectory, withIntermediateDirectories: true, attributes: [.protectionKey: FileProtectionType.complete])
            try JSONEncoder().encode(expectedMarker()).write(to: markerURL, options: [.atomic, .completeFileProtection])
            #expect(try readMarker(markerURL) == expectedMarker())
            #expect(try !context.keystore.hasStoredWallets())
            #expect(context.ports.calls == 0)
        } catch {
            Issue.record("Compact public-only QA import or marker verification failed; no credential, wallet material or underlying error is included.")
        }
    }

    private static let simulatorId = "165BFA41-D46D-47DD-930E-698FDA23B0E1"
    private static let identifier = "bsvhome-phase8-compact-public-qa-v1"
    private static let walletName = "Phase8 Compact Observer"
    private static let address = "1NT3xN86b4iHLZrNrBLSqrT91iFSCCSKPy"

    private func validateHost() throws -> URL {
        #if DEBUG && targetEnvironment(simulator)
            guard BsvHomeAppHostMode.isUnitTestHost,
                  ProcessInfo.processInfo.environment["SIMULATOR_UDID"]?.uppercased() == Self.simulatorId,
                  Bundle.main.bundleIdentifier == "com.bsvhome.ios",
                  Bundle.main.bundleURL.lastPathComponent == "BsvHome.app",
                  Bundle.main.executableURL?.lastPathComponent == "BsvHome"
            else { throw Rejection.invalidState }
            let home = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true).resolvingSymlinksInPath()
            let tail = Array(home.pathComponents.suffix(7))
            guard tail.count == 7,
                  Array(tail.prefix(6)) == ["Devices", Self.simulatorId, "data", "Containers", "Data", "Application"],
                  UUID(uuidString: tail[6]) != nil,
                  let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first,
                  support.resolvingSymlinksInPath().path.hasPrefix(home.path + "/"),
                  let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first
            else { throw Rejection.invalidState }
            for name in ["db.sqlite", "keystore"] {
                guard try !exists(documents.appendingPathComponent(name)) else { throw Rejection.invalidState }
            }
            try FileManager.default.createDirectory(at: support, withIntermediateDirectories: true)
            return support
        #else
            throw Rejection.invalidState
        #endif
    }

    private func makeContext(_ support: URL) throws -> Context {
        let configuration: GRDB.Configuration = DB.defaultConfiguration
        let database = try DB(fileURL: support.appendingPathComponent("db.sqlite"), configuration: configuration)
        let keystoreDirectory = support.appendingPathComponent("keystore", isDirectory: true)
        guard try missingOrEmpty(keystoreDirectory) else { throw Rejection.invalidState }
        let keystore = try BsvHomeKeystore.newWithEnvironment(baseDir: keystoreDirectory.path, environment: .wowPay)
        let wallets = WalletStore(db: database)
        let native = BsvHomeCoreWalletStore(store: wallets)
        let preferences = BsvHomeCorePreferencesStore.application()
        let session = BsvHomeWalletSessionService.newWithEnvironment(store: BsvHomeCoreWalletSessionStore(store: preferences), wallets: native, environment: .wowPay)
        let ports = RejectedPorts()
        let gateway = BsvHomeGateway.newWithEnvironment(provider: ports, preferences: preferences, securePreferences: ports, environment: .wowPay)
        let addresses = BsvHomeBsvAddressPool(gateway: gateway, walletStore: native, store: BsvHomeCoreBsvAddressStore(store: BsvAddressPoolStore(db: database)), keystore: keystore, password: ports)
        let appPreferences = BsvHomePreferencesService(store: preferences)
        let service = BsvHomeWalletService(
            keystore: keystore,
            password: ports,
            store: native,
            session: session,
            appPreferences: appPreferences,
            files: BsvHomeCoreFileStore(),
            preferences: BsvHomeWalletPreferencesService(store: BsvHomeCoreWalletPreferencesStore()),
            explorer: BsvHomeExplorerService(preferences: appPreferences),
            addresses: BsvHomeCoreAddressStore(store: AddressStore(db: database)),
            localizer: BsvHomeCoreLocalizer(),
            bsvAddresses: addresses,
        )
        return Context(database: database, wallets: wallets, service: service, keystore: keystore, preferences: preferences, ports: ports)
    }

    private func seedCanonicalAsset(_ context: Context) async throws {
        let configuration = BsvHomeAssetConfigService()
        let canonical = configuration.defaultAssetBasic(asset: configuration.chainAsset(chain: Primitives.Chain.bitcoinSv.rawValue).asset)
        let assets = Store.AssetStore(db: context.database)
        let existing = try assets.getAssets(for: [canonical.asset.id])
        if existing.isEmpty {
            try await BsvHomeCoreAssetStore(assetStore: assets, balanceStore: BalanceStore(db: context.database)).saveAssets(assets: [canonical])
        }
        guard try assets.getAssets(for: [canonical.asset.id]) == [canonical.asset.map()] else { throw Rejection.invalidState }
    }

    private func verify(_ wallet: Primitives.Wallet, context: Context) async throws {
        guard wallet.id == .view(chain: .bitcoinSv, address: Self.address), wallet.name == Self.walletName,
              wallet.type == .view, wallet.source == .import, wallet.accounts.count == 1,
              let account = wallet.accounts.first, account.chain == .bitcoinSv, account.address == Self.address,
              (account.extendedPublicKey ?? "").isEmpty, account.derivationPath.isEmpty,
              try await context.service.secretKind(walletId: wallet.id.id) == nil,
              try !context.keystore.hasStoredWallets(), context.ports.calls == 0
        else { throw Rejection.invalidState }
    }

    private func expectedMarker() -> Marker {
        Marker(identifier: Self.identifier, formatVersion: 1, simulatorId: Self.simulatorId, role: "compactPublic", walletId: Primitives.WalletId.view(chain: .bitcoinSv, address: Self.address).id, walletName: Self.walletName, address: Self.address, secretStored: false, networkRequests: 0, credentialCalls: 0)
    }

    private func readMarker(_ url: URL) throws -> Marker? {
        guard try exists(url) else { return nil }
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        guard attributes[.type] as? FileAttributeType == .typeRegular,
              let bytes = attributes[.size] as? NSNumber, bytes.intValue <= 4096
        else { throw Rejection.invalidState }
        return try JSONDecoder().decode(Marker.self, from: Data(contentsOf: url))
    }

    private func validatePublicDirectory(_ url: URL, marker: Marker?) throws {
        guard try exists(url) else { return }
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        guard attributes[.type] as? FileAttributeType == .typeDirectory,
              try Set(FileManager.default.contentsOfDirectory(atPath: url.path)) == (marker == nil ? [] : ["public-marker.json"])
        else { throw Rejection.invalidState }
    }

    private func missingOrEmpty(_ url: URL) throws -> Bool {
        guard try exists(url) else { return true }
        guard try FileManager.default.attributesOfItem(atPath: url.path)[.type] as? FileAttributeType == .typeDirectory else { throw Rejection.invalidState }
        return try FileManager.default.contentsOfDirectory(atPath: url.path).isEmpty
    }

    private func exists(_ url: URL) throws -> Bool {
        do {
            _ = try FileManager.default.attributesOfItem(atPath: url.path)
            return true
        } catch let error as CocoaError where error.code == .fileReadNoSuchFile || error.code == .fileNoSuchFile {
            return false
        }
    }

    private struct Marker: Codable, Equatable {
        let identifier: String
        let formatVersion: Int
        let simulatorId: String
        let role: String
        let walletId: String
        let walletName: String
        let address: String
        let secretStored: Bool
        let networkRequests: Int
        let credentialCalls: Int
    }

    private struct Context {
        let database: DB
        let wallets: WalletStore
        let service: BsvHomeWalletService
        let keystore: BsvHomeKeystore
        let preferences: BsvHomeCorePreferencesStore
        let ports: RejectedPorts
    }

    private enum Rejection: Error {
        case invalidState
    }

    private final class RejectedPorts: AlienProvider, BsvHomeKeystorePassword, BsvHomeSecureStore, @unchecked Sendable {
        private let lock = NSLock()
        private var count = 0
        var calls: Int { lock.withLock { count } }

        private func reject() throws -> Never {
            lock.withLock { count += 1 }
            throw Rejection.invalidState
        }

        func request(target _: AlienTarget) async throws -> AlienResponse {
            try reject()
        }

        func getPassword(createIfMissing _: Bool) throws -> String {
            try reject()
        }

        func getWalletPassword(walletId _: String) throws -> String? {
            try reject()
        }

        func deleteWalletPassword(walletId _: String) throws {
            try reject()
        }

        func authentication() throws -> BsvHomeKeystoreAuthentication {
            try reject()
        }

        func get(key _: String) throws -> String? {
            try reject()
        }

        func set(key _: String, value _: String) throws {
            try reject()
        }

        func remove(key _: String) throws {
            try reject()
        }
    }
}
