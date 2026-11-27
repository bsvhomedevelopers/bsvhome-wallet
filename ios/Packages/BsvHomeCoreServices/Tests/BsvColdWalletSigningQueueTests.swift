import Foundation
import BsvHomeCore
import BsvHomeCorePrimitives
@testable import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesTestKit
import Testing

struct BsvColdWalletSigningQueueTests {
    @Test(arguments: ["revision", "payload", "missing", "read-error", "cancel"])
    func coldSigningRechecksTheActualReviewedFileAndScopeInsideTheQueue(change: String) async throws {
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
        let fixture = try await BsvColdWalletSigningScopeFixture.make(directory: directory, coldStore: store) { wallet, request, review, scope in
            _ = try await keystore.bsvColdOperation(wallet: wallet, requestJSON: request, reviewJSON: review, scope: scope, onEnqueued: {
                queued.continuation.yield(())
                queued.continuation.finish()
            }) {
                keys.increment()
                return "public-key-boundary"
            }
            throw BsvHomeServiceError.Cancelled
        }
        let scope = try fixture.beginSigning()
        let signing = Task {
            defer { queued.continuation.finish() }
            try await fixture.sign(scope: scope)
        }
        defer { scope.cancel(); signing.cancel() }
        var waiting = queued.stream.makeAsyncIterator()
        try #require(await waiting.next() != nil)
        let before = try #require(await store.get(walletId: fixture.wallet.id.id))
        let file = try #require(FileManager.default.contentsOfDirectory(at: coldDirectory, includingPropertiesForKeys: nil).first)
        switch change {
        case "revision":
            #expect(try await store.compareAndSwap(walletId: fixture.wallet.id.id, expectedRevision: before.revision, state: .init(revision: before.revision + 1, value: before.value)))
        case "payload":
            let replacement: [String: Any] = ["revision": before.revision, "value": before.value + " "]
            try JSONSerialization.data(withJSONObject: replacement).write(to: file, options: .atomic)
        case "missing": try FileManager.default.removeItem(at: file)
        case "read-error":
            try FileManager.default.removeItem(at: file)
            try FileManager.default.createDirectory(at: file, withIntermediateDirectories: false)
        case "cancel": scope.cancel()
        default: Issue.record("Unknown public queue fixture")
        }
        release.signal()
        _ = try await blocker.value
        if change == "read-error" {
            await #expect(throws: BsvHomeServiceError.Store(msg: "Cold wallet state could not be read")) { try await signing.value }
        } else {
            await #expect(throws: BsvHomeServiceError.Cancelled) { try await signing.value }
        }
        #expect(keys.value == 0)
        #expect(fixture.password.getPasswordCallsCount == 0)
        #expect(fixture.networkRequestCount == 0)
        #expect(try scope.outcome().submission == .notSubmitted)
    }

    @Test
    func anUnchangedConsumedRealScopeReachesOneNativeOperationAndCannotBeReplayed() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: MockKeystorePassword(), environment: .wowPay, coldWalletStore: store)
        let keys = Counter()
        let fixture = try await BsvColdWalletSigningScopeFixture.make(directory: directory, coldStore: store) { wallet, request, review, scope in
            _ = try await keystore.bsvColdOperation(wallet: wallet, requestJSON: request, reviewJSON: review, scope: scope) {
                keys.increment()
                return "public-key-boundary"
            }
            throw BsvHomeServiceError.Cancelled
        }
        let scope = try fixture.beginSigning()
        await #expect(throws: BsvHomeServiceError.Cancelled) { try await fixture.sign(scope: scope) }
        #expect(keys.value == 1)
        await #expect(throws: BsvHomeServiceError.Cancelled) { try await fixture.sign(scope: scope) }
        #expect(keys.value == 1)
        #expect(fixture.password.getPasswordCallsCount == 0)
        #expect(fixture.networkRequestCount == 0)
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
