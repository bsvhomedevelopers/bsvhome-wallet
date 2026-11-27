import CryptoKit
import Darwin
import Foundation
import class BsvHomeCore.BsvHomeOfflineIdentityService
import Keychain

actor OfflineIdentityVault {
    private struct Identity: Codable {
        let publicKey: String
        let privateKey: String
        var backupFingerprint: String?
    }

    private let keychain: any Keychain
    private let rootURL: URL?
    private let keyName = "wowpay.cold.offline-identity.v1"
    private let codec = BsvHomeOfflineIdentityService()

    init(keychain: any Keychain, rootURL: URL? = nil) {
        self.rootURL = rootURL
        self.keychain = keychain.accessibility(.whenUnlockedThisDeviceOnly, authenticationPolicy: [])
    }

    func publicState() throws -> OfflineIdentityPublicState? {
        try withStorageLock {
            guard let identity = try identity() else { return nil }
            return try state(identity)
        }
    }

    func create() throws -> OfflineIdentityPublicState {
        try withStorageLock {
            guard try identity() == nil else { throw OfflineIdentityError.keyAlreadyExists }
            let key = P256.Signing.PrivateKey()
            let identity = Identity(publicKey: OfflineIdentityCryptography.encode(key.publicKey.derRepresentation), privateKey: OfflineIdentityCryptography.encode(key.derRepresentation))
            try write(identity)
            return try state(identity)
        }
    }

    func backup(password: String) throws -> String {
        try withStorageLock {
            guard let identity = try identity() else { throw OfflineIdentityError.keyMissing }
            var plaintext = try JSONEncoder().encode(["publicKey": identity.publicKey, "privateKey": identity.privateKey])
            defer { plaintext.resetBytes(in: plaintext.startIndex ..< plaintext.endIndex) }
            return try OfflineIdentityCryptography.encrypt(plaintext: plaintext, password: password)
        }
    }

    func markBackupSaved(fingerprint: String) throws -> OfflineIdentityPublicState {
        try withStorageLock {
            guard var identity = try identity() else { throw OfflineIdentityError.keyMissing }
            guard try codec.publicKeyFingerprint(publicKey: identity.publicKey) == fingerprint else { throw OfflineIdentityError.staleOperation }
            identity.backupFingerprint = fingerprint
            try write(identity)
            return try state(identity)
        }
    }

    func restore(document: String, password: String) throws -> OfflineIdentityPublicState {
        try withStorageLock {
            guard try identity() == nil else { throw OfflineIdentityError.keyAlreadyExists }
            var plaintext = try OfflineIdentityCryptography.decrypt(document: document, password: password)
            defer { plaintext.resetBytes(in: plaintext.startIndex ..< plaintext.endIndex) }
            guard plaintext.count <= 4096 else { throw OfflineIdentityError.invalidDocument }
            let value = try JSONDecoder().decode([String: String].self, from: plaintext)
            guard Set(value.keys) == Set(["publicKey", "privateKey"]), let publicKey = value["publicKey"], let privateKey = value["privateKey"] else { throw OfflineIdentityError.invalidDocument }
            _ = try OfflineIdentityCryptography.validatedKey(publicKey: publicKey, privateKey: privateKey)
            let fingerprint = try codec.publicKeyFingerprint(publicKey: publicKey)
            let identity = Identity(publicKey: publicKey, privateKey: privateKey, backupFingerprint: fingerprint)
            try write(identity)
            return try state(identity)
        }
    }

    func proof(challengeJSON: String) throws -> String {
        try withStorageLock {
            guard let identity = try identity() else { throw OfflineIdentityError.keyMissing }
            let state = try state(identity)
            guard state.backupSaved else { throw OfflineIdentityError.backupRequired }
            let challenge = try codec.reviewChallenge(challengeJson: challengeJSON, publicKey: identity.publicKey)
            let key = try OfflineIdentityCryptography.validatedKey(publicKey: identity.publicKey, privateKey: identity.privateKey)
            let message = Data(challenge.actionHash.utf8)
            let signature = try key.signature(for: message)
            guard key.publicKey.isValidSignature(signature, for: message) else { throw OfflineIdentityError.cryptographyUnavailable }
            return try codec.proofDocument(challengeJson: challengeJSON, publicKey: identity.publicKey, signature: OfflineIdentityCryptography.encode(signature.derRepresentation))
        }
    }

    private func withStorageLock<T>(_ action: () throws -> T) throws -> T {
        let root = try rootURL ?? FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true).appendingPathComponent("WowPayOfflineIdentity", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        var directory = root
        var resourceValues = URLResourceValues()
        resourceValues.isExcludedFromBackup = true
        try directory.setResourceValues(resourceValues)
        #if os(iOS)
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: root.path)
        #endif
        let file = root.appendingPathComponent("identity.lock")
        let descriptor = file.withUnsafeFileSystemRepresentation { path in
            guard let path else { return Int32(-1) }
            return Darwin.open(path, O_CREAT | O_RDWR | O_NOFOLLOW, 0o600)
        }
        guard descriptor >= 0 else { throw OfflineIdentityError.unavailable }
        defer { Darwin.close(descriptor) }
        guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else { throw OfflineIdentityError.unavailable }
        defer { flock(descriptor, LOCK_UN) }
        #if os(iOS)
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: file.path)
        #endif
        return try action()
    }

    private func identity() throws -> Identity? {
        guard var data = try keychain.getData(keyName, ignoringAttributeSynchronizable: false) else { return nil }
        defer { data.resetBytes(in: data.startIndex ..< data.endIndex) }
        guard data.count <= 8192 else { throw OfflineIdentityError.invalidDocument }
        let identity = try JSONDecoder().decode(Identity.self, from: data)
        _ = try OfflineIdentityCryptography.validatedKey(publicKey: identity.publicKey, privateKey: identity.privateKey)
        return identity
    }

    private func write(_ identity: Identity) throws {
        var data = try JSONEncoder().encode(identity)
        defer { data.resetBytes(in: data.startIndex ..< data.endIndex) }
        try keychain.set(data, key: keyName, ignoringAttributeSynchronizable: false)
    }

    private func state(_ identity: Identity) throws -> OfflineIdentityPublicState {
        let fingerprint = try codec.publicKeyFingerprint(publicKey: identity.publicKey)
        return OfflineIdentityPublicState(publicKey: identity.publicKey, fingerprint: fingerprint, backupSaved: identity.backupFingerprint == fingerprint)
    }
}
