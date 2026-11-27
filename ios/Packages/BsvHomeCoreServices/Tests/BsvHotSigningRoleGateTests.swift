import Foundation
import BsvHomeCore
import BsvHomeCorePrimitives
@testable import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesTestKit
import Testing

struct BsvHotSigningRoleGateTests {
    @Test
    @MainActor
    func actualColdMetadataBlocksHotPasswordAccessBeforeReadingCredentials() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
        let wallet = hotWallet()
        #expect(try await store.compareAndSwap(walletId: wallet.id.id, expectedRevision: nil, state: .init(revision: 0, value: "public-cold-marker")))
        let password = MockKeystorePassword(memoryPassword: "public-dummy-password")
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: password, environment: .wowPay, coldWalletStore: store)
        #expect(throws: BsvHomeServiceError.Unsupported(msg: "BSV cold wallets require the cold signing flow")) {
            try keystore.getPassword(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue)
        }
        #expect(password.getPasswordCallsCount == 0)
        let actualStore = keystore.bsvColdWalletStore as AnyObject
        let usesInjectedStore = actualStore === store
        #expect(usesInjectedStore)
    }

    @Test
    @MainActor
    func metadataCreatedDuringPasswordAccessDoesNotReturnTheCredential() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let coldDirectory = directory.appendingPathComponent("cold", isDirectory: true)
        let store = try BsvHomeCoreBsvColdWalletStore(directory: coldDirectory)
        let wallet = hotWallet()
        #expect(try await store.compareAndSwap(walletId: wallet.id.id, expectedRevision: nil, state: .init(revision: 0, value: "public-cold-marker")))
        let file = try #require(FileManager.default.contentsOfDirectory(at: coldDirectory, includingPropertiesForKeys: nil).first)
        let bytes = try Data(contentsOf: file)
        try FileManager.default.removeItem(at: file)
        let errors = Counter()
        let password = MockKeystorePassword(memoryPassword: "public-dummy-password")
        password.onGetPassword = {
            do { try bytes.write(to: file, options: .atomic) }
            catch { errors.increment() }
        }
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: password, environment: .wowPay, coldWalletStore: store)
        #expect(throws: BsvHomeServiceError.Unsupported(msg: "BSV cold wallets require the cold signing flow")) {
            try keystore.getPassword(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue)
        }
        #expect(password.getPasswordCallsCount == 1)
        #expect(errors.value == 0)
        #expect(try store.hasState(walletId: wallet.id.id))
    }

    @Test(arguments: ["state", "corrupt", "read-error"])
    func hotSigningChecksTheSameRealStoreAfterWaitingInTheActualQueue(change: String) async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let coldDirectory = directory.appendingPathComponent("cold", isDirectory: true)
        let store = try BsvHomeCoreBsvColdWalletStore(directory: coldDirectory)
        let password = MockKeystorePassword()
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: password, environment: .wowPay, coldWalletStore: store)
        let wallet = hotWallet()
        let entered = AsyncStream<Void>.makeStream(bufferingPolicy: .bufferingNewest(1))
        let queued = AsyncStream<Void>.makeStream(bufferingPolicy: .bufferingNewest(1))
        let release = DispatchSemaphore(value: 0)
        defer { release.signal() }
        let blocker = Task {
            try await keystore.bsvMultisigOperation(scope: nil) {
                entered.continuation.yield(())
                entered.continuation.finish()
                release.wait()
                return "public-queue-blocker"
            }
        }
        defer { blocker.cancel() }
        var starts = entered.stream.makeAsyncIterator()
        try #require(await starts.next() != nil)
        let keys = Counter()
        let signing = Task {
            defer { queued.continuation.finish() }
            return try await keystore.hotSigningOperation(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue, onEnqueued: {
                queued.continuation.yield(())
                queued.continuation.finish()
            }) {
                keys.increment()
                return "public-key-boundary"
            }
        }
        defer { signing.cancel() }
        var waiting = queued.stream.makeAsyncIterator()
        try #require(await waiting.next() != nil)
        #expect(try await store.compareAndSwap(walletId: wallet.id.id, expectedRevision: nil, state: .init(revision: 0, value: "public-cold-marker")))
        if change != "state" {
            let file = try #require(FileManager.default.contentsOfDirectory(at: coldDirectory, includingPropertiesForKeys: nil).first)
            if change == "corrupt" {
                try Data("{".utf8).write(to: file, options: .atomic)
            } else {
                try FileManager.default.removeItem(at: file)
                try FileManager.default.createDirectory(at: file, withIntermediateDirectories: false)
            }
        }
        release.signal()
        _ = try await blocker.value
        if change == "state" {
            await #expect(throws: BsvHomeServiceError.Unsupported(msg: "BSV cold wallets require the cold signing flow")) { try await signing.value }
        } else {
            await #expect(throws: BsvHomeServiceError.Store(msg: "Cold wallet state could not be read")) { try await signing.value }
        }
        #expect(keys.value == 0)
        #expect(password.getPasswordCallsCount == 0)
    }

    private func temporaryDirectory() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    }

    private func hotWallet() -> Primitives.Wallet {
        .mock(id: .single(chain: .bitcoinSv, address: "1BitcoinEaterAddressDontSendf59kuE"), type: .single, accounts: [.mock(chain: .bitcoinSv, address: "1BitcoinEaterAddressDontSendf59kuE")])
    }

    private final class Counter: @unchecked Sendable {
        private let lock = NSLock()
        private var count = 0
        var value: Int { lock.withLock { count } }
        func increment() {
            lock.withLock { count += 1 }
        }
    }
}
