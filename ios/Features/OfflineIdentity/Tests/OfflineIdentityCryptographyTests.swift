import CryptoKit
import Foundation
import class BsvHomeCore.BsvHomeOfflineIdentityService
@testable import OfflineIdentity
import Testing

@Suite(.serialized)
struct OfflineIdentityCryptographyTests {
    private struct Vector: Decodable {
        let password: String
        let derivedKey: String
        let publicKey: String
        let privateKey: String
        let plaintext: String
        let backup: String
        let actionHash: String
        let signature: String
    }

    private func vector() throws -> Vector {
        let url = try #require(Bundle.module.url(forResource: "offline-identity-python-vector", withExtension: "json", subdirectory: "Fixtures"))
        return try JSONDecoder().decode(Vector.self, from: Data(contentsOf: url))
    }

    @Test
    func independentUtf8Pbkdf2AesGcmAndPkcs8Vector() throws {
        let vector = try vector()
        let backup = try BsvHomeOfflineIdentityService().readBackup(document: vector.backup)
        let salt = try OfflineIdentityCryptography.decode(backup.salt)
        let nonce = try OfflineIdentityCryptography.decode(backup.nonce)
        let derived = try OfflineIdentityCryptography.derive(password: vector.password, salt: salt)
        #expect(derived.withUnsafeBytes { $0.map { String(format: "%02x", $0) }.joined() } == vector.derivedKey)
        let plaintext = try OfflineIdentityCryptography.decrypt(document: vector.backup, password: vector.password)
        #expect(plaintext == Data(vector.plaintext.utf8))
        let encrypted = try OfflineIdentityCryptography.encrypt(plaintext: Data(vector.plaintext.utf8), password: vector.password, salt: salt, nonce: nonce)
        #expect(try BsvHomeOfflineIdentityService().readBackup(document: encrypted).ciphertext == backup.ciphertext)
        let key = try OfflineIdentityCryptography.validatedKey(publicKey: vector.publicKey, privateKey: vector.privateKey)
        #expect(OfflineIdentityCryptography.encode(key.publicKey.derRepresentation) == vector.publicKey)
        let signature = try P256.Signing.ECDSASignature(derRepresentation: OfflineIdentityCryptography.decode(vector.signature))
        #expect(key.publicKey.isValidSignature(signature, for: Data(vector.actionHash.utf8)))
        #expect(key.publicKey.isValidSignature(signature, for: Data(repeating: 0, count: 32)) == false)
    }

    @Test
    func wrongPasswordTamperedBackupAndChangedPublicKeyAreRejected() throws {
        let vector = try vector()
        #expect(throws: (any Error).self) {
            try OfflineIdentityCryptography.decrypt(document: vector.backup, password: "wrong-password-2026")
        }
        var changed = try #require(JSONSerialization.jsonObject(with: Data(vector.backup.utf8)) as? [String: Any])
        changed["iterations"] = 1
        let changedText = try #require(String(data: JSONSerialization.data(withJSONObject: changed), encoding: .utf8))
        #expect(throws: (any Error).self) { try OfflineIdentityCryptography.decrypt(document: changedText, password: vector.password) }
        changed["iterations"] = 310_000
        changed["ciphertext"] = OfflineIdentityCryptography.encode(Data(repeating: 0, count: 32))
        let tampered = try #require(String(data: JSONSerialization.data(withJSONObject: changed), encoding: .utf8))
        #expect(throws: (any Error).self) { try OfflineIdentityCryptography.decrypt(document: tampered, password: vector.password) }
        let otherKey = P256.Signing.PrivateKey()
        #expect(throws: OfflineIdentityError.invalidDocument) {
            try OfflineIdentityCryptography.validatedKey(publicKey: OfflineIdentityCryptography.encode(otherKey.publicKey.derRepresentation), privateKey: vector.privateKey)
        }
    }

    @Test
    func unbackedIdentityCannotSignAndExistingIdentityCannotBeOverwritten() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let vault = OfflineIdentityVault(keychain: OfflineIdentityKeychainMock(), rootURL: root)
        let state = try await vault.create()
        let document: [String: Any] = ["version": 1, "kind": "wowpay-offline-identity-challenge", "id": String(repeating: "1", count: 32), "actionHash": String(repeating: "2", count: 64), "purpose": "Enroll", "expiresAt": "2026-11-03T00:00:00Z", "userId": "42", "label": "Test offline factor", "fingerprint": state.fingerprint]
        let challenge = try #require(String(data: JSONSerialization.data(withJSONObject: document), encoding: .utf8))
        await #expect(throws: OfflineIdentityError.backupRequired) { try await vault.proof(challengeJSON: challenge) }
        await #expect(throws: OfflineIdentityError.keyAlreadyExists) { try await vault.create() }
        await #expect(throws: OfflineIdentityError.staleOperation) { try await vault.markBackupSaved(fingerprint: String(repeating: "0", count: 64)) }
        _ = try await vault.markBackupSaved(fingerprint: state.fingerprint)
        let proof = try await vault.proof(challengeJSON: challenge)
        let value = try #require(JSONSerialization.jsonObject(with: Data(proof.utf8)) as? [String: Any])
        #expect(value["id"] as? String == document["id"] as? String)
        #expect(value["actionHash"] as? String == document["actionHash"] as? String)
        let signature = try P256.Signing.ECDSASignature(derRepresentation: OfflineIdentityCryptography.decode(#require(value["signature"] as? String)))
        let publicKey = try P256.Signing.PublicKey(derRepresentation: OfflineIdentityCryptography.decode(state.publicKey))
        #expect(publicKey.isValidSignature(signature, for: Data(String(repeating: "2", count: 64).utf8)))
        #expect(publicKey.isValidSignature(signature, for: Data(repeating: 0x22, count: 32)) == false)
    }

    @Test
    @MainActor
    func cancelledSaveAndBackgroundCallbackDoNotAuthorizePublicKeyExport() async {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let model = OfflineIdentityViewModel(keychain: OfflineIdentityKeychainMock(), rootURL: root, authenticate: {})
        model.sceneChanged(.active)
        await model.create()
        #expect(model.publicState?.backupSaved == false)
        model.password = "backup-password-2026"
        model.passwordConfirmation = model.password
        await model.exportBackup()
        #expect(model.pendingExport != nil)
        await model.exported(.failure(CocoaError(.userCancelled)))
        #expect(model.publicState?.backupSaved == false)
        model.exportPublicKey()
        #expect(model.pendingExport == nil)
        model.password = "backup-password-2026"
        model.passwordConfirmation = model.password
        await model.exportBackup()
        model.sceneChanged(.background)
        await model.exported(.success(root.appendingPathComponent("late-export.json")))
        #expect(model.publicState?.backupSaved == false)
        #expect(model.password.isEmpty)
        #expect(model.pendingExport == nil)
    }
}
