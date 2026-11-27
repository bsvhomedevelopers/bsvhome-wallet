import Foundation
import BsvHomeCore
@testable import BsvHomeCoreServices
import Testing

struct BsvColdWalletStateStampTests {
    @Test
    func theStampUsesTheExactValueUtf8AndActualRevision() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        let value = "public:e\u{301}:🔐"
        #expect(try store.stateStamp(walletId: "wallet") == nil)
        #expect(try await store.compareAndSwap(walletId: "wallet", expectedRevision: nil, state: .init(revision: 0, value: value)))
        #expect(try store.stateStamp(walletId: "wallet") == BsvHomeBsvColdWalletStateStamp(revision: 0, payloadHash: "a33f50bc5dd871a70a078aeb2afb81354c5242fa01966eae7506094051af875a"))
        #expect(try await store.get(walletId: "wallet")?.value.utf8.elementsEqual(value.utf8) == true)
        let reopened = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        #expect(try reopened.stateStamp(walletId: "wallet") == store.stateStamp(walletId: "wallet"))
        #expect(try reopened.stateStamp(walletId: "another-wallet") == nil)
    }

    @Test
    func anotherInstanceAndSameRevisionReplacementCannotReuseAStamp() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        let second = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        #expect(try await first.compareAndSwap(walletId: "wallet", expectedRevision: nil, state: .init(revision: 0, value: "public pending")))
        let before = try #require(try first.stateStamp(walletId: "wallet"))
        #expect(try await second.compareAndSwap(walletId: "wallet", expectedRevision: 0, state: .init(revision: 1, value: "public signed")))
        let changed = try #require(try first.stateStamp(walletId: "wallet"))
        #expect(changed.revision == 1)
        #expect(changed.payloadHash != before.payloadHash)
        let file = try #require(FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first)
        let replacement = BsvHomeBsvColdWalletStoredState(revision: 1, value: "public changed full record")
        let envelope: [String: Any] = ["revision": replacement.revision, "value": replacement.value]
        try JSONSerialization.data(withJSONObject: envelope).write(to: file, options: .atomic)
        let replaced = try #require(try second.stateStamp(walletId: "wallet"))
        #expect(replaced.revision == changed.revision)
        #expect(replaced.payloadHash != changed.payloadHash)
        #expect(try first.stateStamp(walletId: "wallet") == replaced)
        try FileManager.default.removeItem(at: file)
        #expect(try first.stateStamp(walletId: "wallet") == nil)
    }

    @Test(arguments: ["malformed", "oversized", "directory"])
    func unreadableRealStateFailsInsteadOfReturningAnAbsentStamp(failure: String) async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory)
        #expect(try await store.compareAndSwap(walletId: "wallet", expectedRevision: nil, state: .init(revision: 0, value: "public pending")))
        let file = try #require(FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first)
        switch failure {
        case "malformed": try Data("{".utf8).write(to: file, options: .atomic)
        case "oversized": try Data(repeating: 0, count: 5_242_881).write(to: file, options: .atomic)
        case "directory":
            try FileManager.default.removeItem(at: file)
            try FileManager.default.createDirectory(at: file, withIntermediateDirectories: false)
        default: Issue.record("Unknown public state-stamp fixture")
        }
        #expect(throws: BsvHomeServiceError.Store(msg: "Cold wallet state could not be read")) { try store.stateStamp(walletId: "wallet") }
        #expect(FileManager.default.fileExists(atPath: file.path))
    }
}
