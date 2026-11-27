import Foundation
import Keychain
import Testing
@testable import WowPayMpc

struct WowPayMpcStorageTests {
    @Test
    func missingSealKeyPreservesEncryptedState() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let keychain = WowPayMpcKeychainMock()
        let storage = try WowPayMpcStorage(walletId: "wallet-1", namespace: "https://example.invalid/v2/api/user-1", keychain: keychain, rootURL: root)
        let key = try storage.withSealKey { $0 }
        #expect(key.count == 32)
        #expect(keychain.access?.rawValue == Accessibility.whenUnlockedThisDeviceOnly.rawValue)
        #expect(keychain.synchronizedAccess == false)
        let encrypted = storage.directory.appendingPathComponent("participant.share")
        let original = Data([1, 2, 3, 4])
        try original.write(to: encrypted)
        keychain.removeAll()
        #expect(throws: WowPayMpcError(code: "MPC_SEAL_KEY_MISSING")) {
            try storage.withSealKey { _ in }
        }
        #expect(try Data(contentsOf: encrypted) == original)
    }

    @Test
    func walletAndAccountNamespacesKeepIndependentSealKeys() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let keychain = WowPayMpcKeychainMock()
        let first = try WowPayMpcStorage(walletId: "wallet-1", namespace: "environment-one/user-1", keychain: keychain, rootURL: root)
        let otherAccount = try WowPayMpcStorage(walletId: "wallet-1", namespace: "environment-one/user-2", keychain: keychain, rootURL: root)
        let otherWallet = try WowPayMpcStorage(walletId: "wallet-2", namespace: "environment-one/user-1", keychain: keychain, rootURL: root)
        let key = try first.withSealKey { $0 }
        #expect(try first.withSealKey { $0 } == key)
        #expect(try otherAccount.withSealKey { $0 } != key)
        #expect(try otherWallet.withSealKey { $0 } != key)
        #expect(try first.directory.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == true)
    }

    @Test
    func nativeActorRejectsConcurrentWalletOpenAndCanCloseAndReopen() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let keychain = WowPayMpcKeychainMock()
        let first = try await WowPayMpcParticipant.open(walletId: "wallet-1", namespace: "isolated-test", keychain: keychain, rootURL: root)
        await #expect(throws: WowPayMpcError(code: "MPC_PARTICIPANT_ALREADY_OPEN")) {
            try await WowPayMpcParticipant.open(walletId: "wallet-1", namespace: "isolated-test", keychain: keychain, rootURL: root)
        }
        try await first.close()
        let reopened = try await WowPayMpcParticipant.open(walletId: "wallet-1", namespace: "isolated-test", keychain: keychain, rootURL: root)
        await #expect(throws: WowPayMpcError(code: "NOT_INITIALIZED")) {
            try await reopened.exchange("{\"op\":\"status\"}")
        }
        try await reopened.close()
        await #expect(throws: WowPayMpcError(code: "MPC_PARTICIPANT_CLOSED")) {
            try await reopened.exchange("{\"op\":\"status\"}")
        }
    }
}
