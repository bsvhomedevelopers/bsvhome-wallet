import Foundation
@testable import BsvHomeCoreServices
import Testing

struct BsvMultisigStoreTests {
    @Test
    func persistsExactPayloadAndRejectsStaleRevision() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvMultisigStore(origin: "https://example.invalid", localWalletId: "wallet", directory: directory)
        #expect(try store.getState() == nil)
        #expect(try await store.compareAndSwap(expectedRevision: 0, json: "frozen payload"))
        #expect(try await !store.compareAndSwap(expectedRevision: 0, json: "replacement"))
        let restored = try BsvHomeCoreBsvMultisigStore(origin: "https://example.invalid", localWalletId: "wallet", directory: directory)
        #expect(try restored.getState()?.json == "frozen payload")
        #expect(try restored.getState()?.revision == 1)
        #expect(try directory.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == true)
        let file = try #require(FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first)
        #expect(try file.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == true)
    }

    @Test(.enabled(if: BsvMultisigStoreTests.isPhysicalIOSDevice, "Complete file protection attributes require a physical iOS device."))
    func fileAndDirectoryUseCompleteProtection() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvMultisigStore(origin: "https://example.invalid", localWalletId: "wallet", directory: directory)
        #expect(try await store.compareAndSwap(expectedRevision: 0, json: "frozen payload"))
        let file = try #require(FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first)
        let fileProtection = try #require(FileManager.default.attributesOfItem(atPath: file.path)[.protectionKey] as? String)
        let directoryProtection = try #require(FileManager.default.attributesOfItem(atPath: directory.path)[.protectionKey] as? String)
        #expect(fileProtection == FileProtectionType.complete.rawValue)
        #expect(directoryProtection == FileProtectionType.complete.rawValue)
    }

    @Test
    func isolatesWalletAndOriginWithoutReadingLegacyStore() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = try BsvHomeCoreBsvMultisigStore(origin: "https://one.invalid", localWalletId: "first", directory: directory)
        #expect(try await first.compareAndSwap(expectedRevision: 0, json: "first identity"))
        let otherWallet = try BsvHomeCoreBsvMultisigStore(origin: "https://one.invalid", localWalletId: "second", directory: directory)
        let otherOrigin = try BsvHomeCoreBsvMultisigStore(origin: "https://two.invalid", localWalletId: "first", directory: directory)
        #expect(try otherWallet.getState() == nil)
        #expect(try otherOrigin.getState() == nil)
    }

    @Test
    func competingInstancesCannotOverwriteSameRevision() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = try BsvHomeCoreBsvMultisigStore(origin: "https://example.invalid", localWalletId: "wallet", directory: directory)
        let second = try BsvHomeCoreBsvMultisigStore(origin: "https://example.invalid", localWalletId: "wallet", directory: directory)
        async let one = first.compareAndSwap(expectedRevision: 0, json: "one")
        async let two = second.compareAndSwap(expectedRevision: 0, json: "two")
        let results = try await [one, two]
        #expect(results.filter(\.self).count == 1)
        #expect(try first.getState()?.revision == 1)
    }

    @Test
    func corruptPayloadAndSaveFailureRemainErrors() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvMultisigStore(origin: "https://example.invalid", localWalletId: "wallet", directory: directory)
        #expect(try await store.compareAndSwap(expectedRevision: 0, json: "saved"))
        let file = try #require(FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first)
        try Data("corrupt".utf8).write(to: file)
        #expect(throws: (any Error).self) { try store.getState() }
        let blocked = directory.appendingPathComponent("file")
        try Data().write(to: blocked)
        let invalid = try BsvHomeCoreBsvMultisigStore(origin: "https://example.invalid", localWalletId: "wallet", directory: blocked)
        await #expect(throws: (any Error).self) { try await invalid.compareAndSwap(expectedRevision: 0, json: "raw") }
    }

    private func temporaryDirectory() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    }

    private static var isPhysicalIOSDevice: Bool {
        #if os(iOS) && !targetEnvironment(simulator)
            true
        #else
            false
        #endif
    }
}
