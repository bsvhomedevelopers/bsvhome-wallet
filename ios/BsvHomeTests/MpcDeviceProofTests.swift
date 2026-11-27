import CryptoKit
import Foundation
@testable import BsvHomeCoreServices
import Security
import Testing

struct MpcDeviceProofTests {
    @Test
    func persistentDeviceKeySignsUtf8ActionHashWithSpkiAndDer() async throws {
        let namespace = "isolated-device-proof-" + UUID().uuidString
        let installation = UUID().uuidString
        let tag = Data("WowPay.MPC.Device.".utf8) + Data(SHA256.hash(data: Data((namespace + "\n" + installation).utf8)))
        defer { SecItemDelete([kSecClass: kSecClassKey, kSecAttrApplicationTag: tag] as CFDictionary) }
        let status = SecItemCopyMatching([kSecClass: kSecClassKey, kSecAttrApplicationTag: tag] as CFDictionary, nil)
        try #require(status == errSecItemNotFound, "Isolated device-key Keychain query status: \(status)")
        let proof = BsvHomeCoreMpcDeviceProof(namespace: namespace, installationId: installation)
        let keys = try await withThrowingTaskGroup(of: String.self) { group in
            for _ in 0 ..< 8 {
                group.addTask { try await proof.publicKey() }
            }
            var keys = Set<String>()
            for try await key in group {
                keys.insert(key)
            }
            return keys
        }
        #expect(keys.count == 1)
        let publicKey = try #require(keys.first)
        let message = String(repeating: "ab", count: 32)
        let signature = try await P256.Signing.ECDSASignature(derRepresentation: PlatformWebAuthnOptions.binary(proof.signMessage(message: message)))
        let verifier = try P256.Signing.PublicKey(derRepresentation: PlatformWebAuthnOptions.binary(publicKey))
        #expect(verifier.isValidSignature(signature, for: Data(message.utf8)))
        #expect(!verifier.isValidSignature(signature, for: Data(repeating: 0xAB, count: 32)))
        let reopened = BsvHomeCoreMpcDeviceProof(namespace: namespace, installationId: installation)
        #expect(try await reopened.publicKey() == publicKey)
    }
}
