import Foundation
import BsvHomeCore
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
@testable import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import Testing

struct BsvMultisigColdKeyGateTests {
    @Test(arguments: ["proposal", "funding", "authenticate", "authenticateLegacy"])
    func onlineFinancialAndAuthenticationEntrypointsRejectColdMetadataBeforeReadingPassword(entry: String) async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
        #expect(try await store.compareAndSwap(walletId: fixture.payerWalletId, expectedRevision: nil, state: .init(revision: 0, value: "public-cold-routing-marker")))
        let password = MockKeystorePassword(memoryPassword: LocalKeystore.password)
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: password, environment: .wowPay, coldWalletStore: store)
        let signer = BsvHomeCoreBsvMultisigSigner(keystore: keystore)
        try scope.check()
        await #expect(throws: BsvHomeServiceError.Unsupported(msg: "BSV cold wallets require the cold signing flow")) {
            switch entry {
            case "proposal":
                _ = try await signer.signProposal(wallet: fixture.payer, policyJson: "{}", proposalJson: "{}", recipient: "1BitcoinEaterAddressDontSendf59kuE", amount: 1, maximumFee: 1, scope: scope)
            case "funding":
                _ = try await signer.signFunding(wallet: fixture.payer, fundingJson: "{}", reviewJson: "{}", scope: scope)
            case "authenticate":
                try await signer.authenticate(wallet: fixture.payer, scope: scope)
            case "authenticateLegacy":
                try await signer.authenticate(wallet: fixture.payer, scope: nil)
            default: Issue.record("Unknown public entry fixture")
            }
        }
        #expect(password.getPasswordCallsCount == 0)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test
    @MainActor
    func metadataCreatedDuringOnlinePasswordAccessDoesNotReturnTheCredential() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let coldDirectory = directory.appendingPathComponent("cold", isDirectory: true)
        let store = try BsvHomeCoreBsvColdWalletStore(directory: coldDirectory)
        #expect(try await store.compareAndSwap(walletId: fixture.payerWalletId, expectedRevision: nil, state: .init(revision: 0, value: "public-cold-routing-marker")))
        let file = try #require(FileManager.default.contentsOfDirectory(at: coldDirectory, includingPropertiesForKeys: nil).first)
        let envelope = try Data(contentsOf: file)
        try FileManager.default.removeItem(at: file)
        let failures = Counter()
        let password = MockKeystorePassword(memoryPassword: LocalKeystore.password)
        password.onGetPassword = {
            do { try envelope.write(to: file, options: .atomic) }
            catch { failures.increment() }
        }
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: password, environment: .wowPay, coldWalletStore: store)
        try scope.check()
        #expect(throws: BsvHomeServiceError.Unsupported(msg: "BSV cold wallets require the cold signing flow")) {
            try keystore.getPassword(wallet: fixture.payer.map(), scope: scope)
        }
        #expect(password.getPasswordCallsCount == 1)
        #expect(failures.value == 0)
        #expect(try store.hasState(walletId: fixture.payerWalletId))
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test(arguments: ["state", "corrupt", "read-error"])
    func theActualOnlineMultisigQueueRechecksTheNativeStoreAfterEnqueue(change: String) async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let coldDirectory = directory.appendingPathComponent("cold", isDirectory: true)
        let store = try BsvHomeCoreBsvColdWalletStore(directory: coldDirectory)
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: MockKeystorePassword(), environment: .wowPay, coldWalletStore: store)
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
            return try await keystore.bsvMultisigSigningOperation(wallet: fixture.payer.map(), scope: scope, onEnqueued: {
                queued.continuation.yield(())
                queued.continuation.finish()
            }) {
                keys.increment()
                return "public-key-boundary"
            }
        }
        defer { scope.cancel(); signing.cancel() }
        var waiting = queued.stream.makeAsyncIterator()
        try #require(await waiting.next() != nil)
        #expect(try await store.compareAndSwap(walletId: fixture.payerWalletId, expectedRevision: nil, state: .init(revision: 0, value: "public-cold-routing-marker")))
        if change != "state" {
            let file = try #require(FileManager.default.contentsOfDirectory(at: coldDirectory, includingPropertiesForKeys: nil).first)
            if change == "corrupt" {
                try Data("{".utf8).write(to: file, options: .atomic)
            } else {
                try FileManager.default.removeItem(at: file)
                try FileManager.default.createDirectory(at: file, withIntermediateDirectories: false)
            }
        }
        try scope.check()
        release.signal()
        _ = try await blocker.value
        if change == "state" {
            await #expect(throws: BsvHomeServiceError.Unsupported(msg: "BSV cold wallets require the cold signing flow")) { try await signing.value }
        } else {
            await #expect(throws: BsvHomeServiceError.Store(msg: "Cold wallet state could not be read")) { try await signing.value }
        }
        #expect(keys.value == 0)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test
    func aLiveScopeAndAnUnmarkedNativeWalletReachTheExistingQueueBoundary() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: MockKeystorePassword(), environment: .wowPay, coldWalletStore: store)
        let keys = Counter()
        #expect(try await keystore.bsvMultisigSigningOperation(wallet: fixture.payer.map(), scope: scope) {
            keys.increment()
            return "public-key-boundary"
        } == "public-key-boundary")
        #expect(keys.value == 1)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    private func temporaryDirectory() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
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
