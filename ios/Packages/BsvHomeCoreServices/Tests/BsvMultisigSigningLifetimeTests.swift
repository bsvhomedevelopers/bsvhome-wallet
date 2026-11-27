import Foundation
import BsvHomeCore
import BsvHomeCorePrimitivesTestKit
@testable import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Testing

struct BsvMultisigSigningLifetimeTests {
    @Test
    func theActualKeystoreQueueRejectsACancelledScopeBeforeItsOperation() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        let calls = OperationCount()
        scope.cancel()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let keystore = try LocalKeystore(directoryURL: directory, keystorePassword: MockKeystorePassword(), environment: .wowPay)
        await #expect(throws: BsvHomeBsvMultisigError.Cancelled) {
            try await keystore.bsvMultisigOperation(scope: scope) {
                calls.increment()
                return "public-operation-result"
            }
        }
        #expect(calls.value == 0)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test
    func cancellationWhileAQueuedOperationIsRunningDiscardsItsResult() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        let entered = AsyncStream<Void>.makeStream()
        let resume = DispatchSemaphore(value: 0)
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let keystore = try LocalKeystore(directoryURL: directory, keystorePassword: MockKeystorePassword(), environment: .wowPay)
        let operation = Task {
            try await keystore.bsvMultisigOperation(scope: scope) {
                entered.continuation.yield(())
                resume.wait()
                return "public-operation-result"
            }
        }
        var entries = entered.stream.makeAsyncIterator()
        try #require(await entries.next() != nil)
        scope.cancel()
        resume.signal()
        await #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try await operation.value }
    }

    @Test
    func scopeCancellationDoesNotPoisonTheKeystoreOrAnotherAction() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let cancelled = try fixture.beginRetryFunding()
        let next = try fixture.beginRetryFunding()
        cancelled.cancel()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let keystore = try LocalKeystore(directoryURL: directory, keystorePassword: MockKeystorePassword(), environment: .wowPay)
        await #expect(throws: BsvHomeBsvMultisigError.Cancelled) {
            try await keystore.bsvMultisigOperation(scope: cancelled) { "cancelled" }
        }
        #expect(try await keystore.bsvMultisigOperation(scope: next) { "next-public-result" } == "next-public-result")
    }

    @Test
    @MainActor
    func cancellationDuringPasswordAccessDoesNotReturnThePassword() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        let password = MockKeystorePassword(memoryPassword: "dummy-password")
        password.onGetPassword = { scope.cancel() }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let keystore = try LocalKeystore(directoryURL: directory, keystorePassword: password, environment: .wowPay)
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try keystore.getPassword(scope: scope) }
        #expect(password.getPasswordCallsCount == 1)
    }

    @Test(arguments: ["state", "invalid", "read-error"])
    func coldMetadataWrittenAfterEnqueuePreventsTheKeyOperation(failure: String) async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let coldStore = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
        let fixture = try await BsvMultisigActionScopeFixture.make(coldStore: coldStore)
        let scope = try fixture.beginRetryFunding()
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: MockKeystorePassword(), environment: .wowPay)
        let blockerEntered = AsyncStream<Void>.makeStream(bufferingPolicy: .bufferingNewest(1))
        let queued = AsyncStream<Void>.makeStream(bufferingPolicy: .bufferingNewest(1))
        let resume = DispatchSemaphore(value: 0)
        defer { scope.cancel(); resume.signal() }
        let blocker = Task {
            try await keystore.bsvMultisigOperation(scope: nil) {
                blockerEntered.continuation.yield(())
                blockerEntered.continuation.finish()
                resume.wait()
                return "public-queue-blocker"
            }
        }
        defer { blocker.cancel() }
        var starts = blockerEntered.stream.makeAsyncIterator()
        try #require(await starts.next() != nil)
        let calls = OperationCount()
        let operation = Task {
            try await keystore.bsvMultisigOperation(scope: scope, onEnqueued: {
                queued.continuation.yield(())
                queued.continuation.finish()
            }) {
                calls.increment()
                return "public-key-operation-result"
            }
        }
        defer { operation.cancel() }
        var enqueues = queued.stream.makeAsyncIterator()
        try #require(await enqueues.next() != nil)
        #expect(calls.value == 0)
        #expect(try await coldStore.compareAndSwap(walletId: fixture.payerWalletId, expectedRevision: nil, state: .init(revision: 0, value: "public-cold-state")))
        if failure != "state" {
            let file = try #require(FileManager.default.contentsOfDirectory(at: directory.appendingPathComponent("cold", isDirectory: true), includingPropertiesForKeys: nil).first)
            if failure == "invalid" {
                try Data("{".utf8).write(to: file, options: .atomic)
            } else {
                try FileManager.default.removeItem(at: file)
                try FileManager.default.createDirectory(at: file, withIntermediateDirectories: false)
            }
        }
        resume.signal()
        _ = try await blocker.value
        if failure == "state" {
            await #expect(throws: BsvHomeBsvMultisigError.FundingPayerMissing) { try await operation.value }
        } else {
            await #expect(throws: BsvHomeBsvMultisigError.Storage(reason: "BSV_PUBLIC_PAYER_METADATA_UNAVAILABLE")) { try await operation.value }
        }
        #expect(calls.value == 0)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    private final class OperationCount: @unchecked Sendable {
        private let lock = NSLock()
        private var count = 0
        var value: Int { lock.withLock { count } }
        func increment() {
            lock.withLock { count += 1 }
        }
    }
}
