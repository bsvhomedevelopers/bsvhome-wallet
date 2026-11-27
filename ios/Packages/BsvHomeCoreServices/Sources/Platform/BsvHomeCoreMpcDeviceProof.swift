import CryptoKit
import Foundation
import protocol BsvHomeCore.BsvHomeMpcDeviceProof
import enum BsvHomeCore.BsvHomeMpcError
import Security

public actor BsvHomeCoreMpcDeviceProof: BsvHomeMpcDeviceProof {
    private let applicationTag: Data

    public init(namespace: String, installationId: String) {
        applicationTag = Data("WowPay.MPC.Device.".utf8) + Data(SHA256.hash(data: Data((namespace + "\n" + installationId).utf8)))
    }

    public func publicKey() async throws -> String {
        let key = try Self.key(tag: applicationTag)
        guard let publicKey = SecKeyCopyPublicKey(key) else { throw BsvHomeMpcError.SecureStorageUnavailable }
        var error: Unmanaged<CFError>?
        guard let external = SecKeyCopyExternalRepresentation(publicKey, &error) as Data? else { throw BsvHomeMpcError.SecureStorageUnavailable }
        let publicKeyData = try P256.Signing.PublicKey(x963Representation: external).derRepresentation
        return PlatformWebAuthnOptions.encoded(publicKeyData)
    }

    public func signMessage(message: String) async throws -> String {
        let key = try Self.key(tag: applicationTag)
        let algorithm = SecKeyAlgorithm.ecdsaSignatureMessageX962SHA256
        guard SecKeyIsAlgorithmSupported(key, .sign, algorithm) else { throw BsvHomeMpcError.SecureStorageUnavailable }
        var error: Unmanaged<CFError>?
        guard let signature = SecKeyCreateSignature(key, algorithm, Data(message.utf8) as CFData, &error) as Data? else {
            throw BsvHomeMpcError.SecureStorageUnavailable
        }
        return PlatformWebAuthnOptions.encoded(signature)
    }

    private static func key(tag: Data) throws -> SecKey {
        let query: [CFString: Any] = [kSecClass: kSecClassKey, kSecAttrApplicationTag: tag, kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom, kSecReturnRef: true]
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecSuccess, let result {
            return result as! SecKey
        }
        guard status == errSecItemNotFound else { throw BsvHomeMpcError.SecureStorageUnavailable }
        var error: Unmanaged<CFError>?
        let enclave = SecureEnclave.isAvailable
        guard let access = SecAccessControlCreateWithFlags(nil, kSecAttrAccessibleWhenUnlockedThisDeviceOnly, enclave ? [.privateKeyUsage] : [], &error) else {
            throw BsvHomeMpcError.SecureStorageUnavailable
        }
        var attributes: [CFString: Any] = [
            kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeySizeInBits: 256,
            kSecPrivateKeyAttrs: [kSecAttrIsPermanent: true, kSecAttrApplicationTag: tag, kSecAttrAccessControl: access],
        ]
        if enclave {
            attributes[kSecAttrTokenID] = kSecAttrTokenIDSecureEnclave
        }
        guard let key = SecKeyCreateRandomKey(attributes as CFDictionary, &error) else { throw BsvHomeMpcError.SecureStorageUnavailable }
        return key
    }
}
