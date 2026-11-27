import Foundation
import enum BsvHomeCore.AlienError
import protocol BsvHomeCore.AlienProvider
import class BsvHomeCore.AlienResponse
import struct BsvHomeCore.AlienTarget
import class BsvHomeCore.BsvHomeDeviceKeyService
import class BsvHomeCore.BsvHomeKeystore
import enum BsvHomeCore.BsvHomeKeystoreAuthentication
import protocol BsvHomeCore.BsvHomeKeystorePassword
import class BsvHomeCore.BsvHomePortalAuthService
import class BsvHomeCore.BsvHomePortalTransferPasswordService
import enum BsvHomeCore.BsvHomeServiceError
import enum BsvHomeCore.BsvHomeTransferPasswordError
import class BsvHomeCore.BsvHomeWalletSessionService
import typealias BsvHomeCore.WalletId
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit
import Testing

struct TransferPasswordBridgeTests {
    @Test(.timeLimit(.minutes(1)))
    func watchWalletRejectsStatusThroughNativeStoreWithoutNetworkOrPassword() async throws {
        let memoryURL = try #require(URL(string: "%3Amemory%3A"))
        try #require(memoryURL.path(percentEncoded: false) == ":memory:")
        let db = try DB(fileURL: memoryURL)
        try AssetStore.mock(db: db).add(assets: [.mock(asset: .mock(id: AssetId(chain: .bitcoinSv), name: "Bitcoin SV", symbol: "BSV"))])
        let walletStore = WalletStore.mock(db: db)
        let address = "1BitcoinEaterAddressDontSendf59kuE"
        let wallet = Primitives.Wallet.mock(
            id: .make(walletType: .view, chain: .bitcoinSv, address: address),
            name: "Public watch bridge fixture",
            type: .view,
            accounts: [.mock(chain: .bitcoinSv, address: address)],
            source: .import,
        )
        try walletStore.addWallet(wallet)
        let session = BsvHomeWalletSessionService.newWithEnvironment(
            store: BsvHomeCoreWalletSessionStore.mock(),
            wallets: BsvHomeCoreWalletStore(store: walletStore),
            environment: .wowPay,
        )
        try session.setCurrent(walletId: wallet.id)
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let keystore = try BsvHomeKeystore.newWithEnvironment(baseDir: directory.path, environment: .wowPay)
        let provider = RejectingProvider()
        let password = RejectingPassword()
        let auth = try BsvHomePortalAuthService(
            provider: provider,
            keystore: keystore,
            password: password,
            session: session,
            deviceKey: BsvHomeDeviceKeyService(store: BsvHomeSecureStoreMock()),
            store: BsvHomeSecureStoreMock(),
            baseUrl: "https://paymail.bsvhome.com/api/portal",
            language: "en",
            deviceType: "4",
        )
        let service = try BsvHomePortalTransferPasswordService(auth: auth, walletId: wallet.id.id)

        await #expect(throws: BsvHomeTransferPasswordError.WatchOnly) {
            try await service.status()
        }

        #expect(await provider.requests == 0)
        #expect(password.accesses == 0)
        #expect(try FileManager.default.contentsOfDirectory(atPath: directory.path).isEmpty)
    }

    private actor RejectingProvider: AlienProvider {
        private(set) var requests = 0

        func request(target _: AlienTarget) async throws -> AlienResponse {
            requests += 1
            throw AlienError.RequestError(msg: "Watch-only bridge test forbids network access")
        }
    }

    private final class RejectingPassword: BsvHomeKeystorePassword, @unchecked Sendable {
        private let lock = NSLock()
        private var count = 0

        var accesses: Int {
            lock.withLock { count }
        }

        func getPassword(createIfMissing _: Bool) throws -> String {
            throw failure()
        }

        func getWalletPassword(walletId _: BsvHomeCore.WalletId) throws -> String? {
            throw failure()
        }

        func deleteWalletPassword(walletId _: BsvHomeCore.WalletId) throws {
            throw failure()
        }

        func authentication() throws -> BsvHomeKeystoreAuthentication {
            throw failure()
        }

        private func failure() -> BsvHomeServiceError {
            lock.withLock { count += 1 }
            return .Unsupported(msg: "Watch-only bridge test forbids password access")
        }
    }
}
