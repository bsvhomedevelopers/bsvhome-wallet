import Foundation
import BsvHomeCore
@testable import BsvHomeCoreServices
import Testing

struct BsvColdWalletStoreTests {
    @Test
    func pendingStateSurvivesReopenAndSeparatesWallets() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        #expect(try await first.get(walletId: "wallet-a") == nil)
        #expect(try await first.compareAndSwap(walletId: "wallet-a", expectedRevision: nil, state: .init(revision: 0, value: "request")))
        #expect(try await !first.compareAndSwap(walletId: "wallet-a", expectedRevision: nil, state: .init(revision: 0, value: "replacement")))
        #expect(try await first.compareAndSwap(walletId: "wallet-a", expectedRevision: 0, state: .init(revision: 1, value: "signed")))
        let reopened = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        #expect(try await reopened.get(walletId: "wallet-a")?.value == "signed")
        #expect(try await reopened.get(walletId: "wallet-b") == nil)
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: [.isExcludedFromBackupKey])
        #expect(files.count == 1)
        #expect(try files[0].resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == true)
    }

    @Test
    func corruptPendingCannotBeTreatedAsMissing() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        #expect(try await store.compareAndSwap(walletId: "wallet", expectedRevision: nil, state: .init(revision: 0, value: "request")))
        let file = try #require(FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first)
        try Data("corrupt".utf8).write(to: file, options: .atomic)
        await #expect(throws: (any Error).self) { try await store.get(walletId: "wallet") }
        #expect(FileManager.default.fileExists(atPath: file.path))
    }

    @Test
    func synchronousPresenceUsesTheSameRealFileAcrossInstances() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        let reopened = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        #expect(try !first.hasState(walletId: "wallet-a"))
        #expect(try await first.compareAndSwap(walletId: "wallet-a", expectedRevision: nil, state: .init(revision: 0, value: "request")))
        #expect(try first.hasState(walletId: "wallet-a"))
        #expect(try reopened.hasState(walletId: "wallet-a"))
        #expect(try !reopened.hasState(walletId: "wallet-b"))
        #expect(try await reopened.compareAndSwap(walletId: "wallet-a", expectedRevision: 0, state: .init(revision: 1, value: "signed")))
        #expect(try first.hasState(walletId: "wallet-a"))
        #expect(try await first.get(walletId: "wallet-a")?.value == "signed")
    }

    @Test(arguments: ["malformed", "oversized", "directory"])
    func synchronousPresenceNeverConvertsUnreadableStateToMissing(failure: String) async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        #expect(try await store.compareAndSwap(walletId: "wallet", expectedRevision: nil, state: .init(revision: 0, value: "request")))
        let file = try #require(FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first)
        switch failure {
        case "malformed": try Data("{".utf8).write(to: file, options: .atomic)
        case "oversized": try Data(repeating: 0, count: 5_242_881).write(to: file, options: .atomic)
        case "directory":
            try FileManager.default.removeItem(at: file)
            try FileManager.default.createDirectory(at: file, withIntermediateDirectories: false)
        default: Issue.record("Unknown public storage fixture")
        }
        #expect(throws: BsvHomeServiceError.Store(msg: "Cold wallet state could not be read")) { try store.hasState(walletId: "wallet") }
        #expect(FileManager.default.fileExists(atPath: file.path))
    }
}
