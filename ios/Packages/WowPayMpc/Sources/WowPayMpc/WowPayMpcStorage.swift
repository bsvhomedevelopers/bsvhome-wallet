import CryptoKit
import Darwin
import Foundation
import Keychain
import Security

struct WowPayMpcStorage {
    let directory: URL
    private let keychain: any Keychain
    private let keyName: String

    init(walletId: String, namespace: String, keychain: any Keychain, rootURL: URL?) throws {
        guard !namespace.isEmpty, namespace.utf8.count <= 4096,
              !walletId.isEmpty, walletId.utf8.count <= 64,
              walletId.utf8.allSatisfy({ (48 ... 57).contains($0) || (65 ... 90).contains($0) || (97 ... 122).contains($0) || $0 == 45 || $0 == 95 })
        else {
            throw WowPayMpcError(code: "WALLET_INVALID")
        }
        let namespaceHash = SHA256.hash(data: Data(namespace.utf8)).map { String(format: "%02x", $0) }.joined()
        let walletHash = SHA256.hash(data: Data(walletId.utf8)).map { String(format: "%02x", $0) }.joined()
        let base = try rootURL ?? FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent("WowPayMpc", isDirectory: true)
        directory = base.appendingPathComponent(namespaceHash, isDirectory: true).appendingPathComponent(walletHash, isDirectory: true)
        self.keychain = keychain.accessibility(.whenUnlockedThisDeviceOnly, authenticationPolicy: [])
        keyName = "wowpay.mpc.seal.v1.\(namespaceHash).\(walletHash)"
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        try protectFiles()
    }

    func withSealKey<T>(_ action: (inout Data) throws -> T) throws -> T {
        let descriptor = directory.appendingPathComponent("seal.lock").withUnsafeFileSystemRepresentation { path in
            guard let path else { return Int32(-1) }
            return Darwin.open(path, O_CREAT | O_RDWR | O_NOFOLLOW, 0o600)
        }
        guard descriptor >= 0 else { throw WowPayMpcError(code: "MPC_LOCK_UNAVAILABLE") }
        defer { Darwin.close(descriptor) }
        guard flock(descriptor, LOCK_EX) == 0 else { throw WowPayMpcError(code: "MPC_LOCK_UNAVAILABLE") }
        defer { flock(descriptor, LOCK_UN) }
        var key = try sealKey()
        defer { key.resetBytes(in: key.startIndex ..< key.endIndex) }
        return try action(&key)
    }

    func protectFiles() throws {
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        var directory = directory
        try directory.setResourceValues(values)
        #if os(iOS)
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: directory.path)
            for file in try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil) {
                try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: file.path)
            }
        #endif
    }

    private func sealKey() throws -> Data {
        let existing: Data?
        do {
            existing = try keychain.getData(keyName, ignoringAttributeSynchronizable: false)
        } catch {
            throw WowPayMpcError(code: "MPC_KEYCHAIN_UNAVAILABLE")
        }
        if let existing {
            guard existing.count == 32 else { throw WowPayMpcError(code: "MPC_SEAL_KEY_INVALID") }
            return existing
        }
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        guard !files.contains(where: { $0.lastPathComponent == "participant.share" || $0.lastPathComponent.hasPrefix("participant.pending-") }) else {
            throw WowPayMpcError(code: "MPC_SEAL_KEY_MISSING")
        }
        var key = Data(count: 32)
        let status = key.withUnsafeMutableBytes { bytes in
            guard let address = bytes.baseAddress else { return errSecParam }
            return SecRandomCopyBytes(kSecRandomDefault, bytes.count, address)
        }
        guard status == errSecSuccess else { throw WowPayMpcError(code: "RANDOM_UNAVAILABLE") }
        do {
            try keychain.set(key, key: keyName, ignoringAttributeSynchronizable: false)
        } catch {
            key.resetBytes(in: key.startIndex ..< key.endIndex)
            throw WowPayMpcError(code: "MPC_KEYCHAIN_UNAVAILABLE")
        }
        return key
    }
}
