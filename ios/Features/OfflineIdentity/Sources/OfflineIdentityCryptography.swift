import CommonCrypto
import CryptoKit
import Foundation
import class BsvHomeCore.BsvHomeOfflineIdentityService
import Security

enum OfflineIdentityCryptography {
    static let backupContext = Data("WowPay/OfflineIdentity/backup/v1".utf8)

    static func encode(_ data: Data) -> String {
        data.base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    }

    static func decode(_ text: String) throws -> Data {
        let padded = text.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/") + String(repeating: "=", count: (4 - text.count % 4) % 4)
        guard let data = Data(base64Encoded: padded), encode(data) == text else { throw OfflineIdentityError.invalidDocument }
        return data
    }

    static func derive(password: String, salt: Data) throws -> SymmetricKey {
        guard password.utf16.count >= 12, password.utf8.count <= 4096, salt.count == 16 else { throw OfflineIdentityError.invalidPassword }
        var passwordBytes = Array(password.utf8)
        defer { _ = passwordBytes.withUnsafeMutableBytes { $0.initializeMemory(as: UInt8.self, repeating: 0) } }
        var output = Data(count: 32)
        let status = passwordBytes.withUnsafeBytes { password in
            salt.withUnsafeBytes { salt in
                output.withUnsafeMutableBytes { output in
                    CCKeyDerivationPBKDF(CCPBKDFAlgorithm(kCCPBKDF2), password.bindMemory(to: Int8.self).baseAddress, password.count, salt.bindMemory(to: UInt8.self).baseAddress, salt.count, CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA256), 310_000, output.bindMemory(to: UInt8.self).baseAddress, output.count)
                }
            }
        }
        defer { output.resetBytes(in: output.startIndex ..< output.endIndex) }
        guard status == kCCSuccess else { throw OfflineIdentityError.cryptographyUnavailable }
        return SymmetricKey(data: output)
    }

    static func encrypt(plaintext: Data, password: String, salt: Data? = nil, nonce: Data? = nil) throws -> String {
        let salt = try salt ?? random(count: 16)
        let nonce = try nonce ?? random(count: 12)
        let key = try derive(password: password, salt: salt)
        let box = try AES.GCM.seal(plaintext, using: key, nonce: AES.GCM.Nonce(data: nonce), authenticating: backupContext)
        return try BsvHomeOfflineIdentityService().backupDocument(salt: encode(salt), nonce: encode(nonce), ciphertext: encode(box.ciphertext + box.tag))
    }

    static func decrypt(document: String, password: String) throws -> Data {
        let backup = try BsvHomeOfflineIdentityService().readBackup(document: document)
        let salt = try decode(backup.salt)
        let nonce = try decode(backup.nonce)
        let ciphertext = try decode(backup.ciphertext)
        let key = try derive(password: password, salt: salt)
        let box = try AES.GCM.SealedBox(nonce: AES.GCM.Nonce(data: nonce), ciphertext: ciphertext.dropLast(16), tag: ciphertext.suffix(16))
        return try AES.GCM.open(box, using: key, authenticating: backupContext)
    }

    static func validatedKey(publicKey: String, privateKey: String) throws -> P256.Signing.PrivateKey {
        _ = try BsvHomeOfflineIdentityService().publicKeyFingerprint(publicKey: publicKey)
        var privateBytes = try decode(privateKey)
        defer { privateBytes.resetBytes(in: privateBytes.startIndex ..< privateBytes.endIndex) }
        let secret = try P256.Signing.PrivateKey(derRepresentation: privateBytes)
        let publicBytes = try decode(publicKey)
        let publicKey = try P256.Signing.PublicKey(derRepresentation: publicBytes)
        guard publicKey.derRepresentation == publicBytes, secret.publicKey.derRepresentation == publicBytes else { throw OfflineIdentityError.invalidDocument }
        return secret
    }

    private static func random(count: Int) throws -> Data {
        var data = Data(count: count)
        let status = data.withUnsafeMutableBytes { bytes in
            guard let address = bytes.baseAddress else { return errSecParam }
            return SecRandomCopyBytes(kSecRandomDefault, count, address)
        }
        guard status == errSecSuccess else { throw OfflineIdentityError.cryptographyUnavailable }
        return data
    }
}
