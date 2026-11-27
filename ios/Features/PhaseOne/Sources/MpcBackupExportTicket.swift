import Foundation
import struct BsvHomeCore.BsvHomeMpcBackupMaterial

struct MpcBackupExportTicket: Identifiable, Equatable, Sendable {
    let id: UUID
    let revision: UInt
    let environment: String
    let ownerId: String
    let walletId: String
    let epoch: UInt64
    let commitment: String
    let fileURL: URL

    static func create(revision: UInt, environment: String, ownerId: String, walletId: String, epoch: UInt64, commitment: String, document: String) throws -> Self {
        let id = UUID()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("WowPayMpcBackup-" + id.uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false, attributes: [.protectionKey: FileProtectionType.complete, .posixPermissions: 0o700])
        do {
            var protectedDirectory = directory
            var attributes = URLResourceValues()
            attributes.isExcludedFromBackup = true
            try protectedDirectory.setResourceValues(attributes)
            var fileURL = directory.appendingPathComponent("wowpay-mpc-encrypted-backup.json")
            try Data(document.utf8).write(to: fileURL, options: [.atomic, .completeFileProtection])
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete, .posixPermissions: 0o600], ofItemAtPath: fileURL.path)
            try fileURL.setResourceValues(attributes)
            return Self(id: id, revision: revision, environment: environment, ownerId: ownerId, walletId: walletId, epoch: epoch, commitment: commitment, fileURL: fileURL)
        } catch {
            try FileManager.default.removeItem(at: directory)
            throw error
        }
    }

    func matches(_ material: BsvHomeMpcBackupMaterial, revision: UInt, walletId: String?) -> Bool {
        self.revision == revision && self.walletId == walletId && self.walletId == material.walletId
            && environment == material.environment && ownerId == material.ownerId && epoch == material.epoch && commitment == material.commitment
    }

    func removeTemporaryFile() throws {
        let directory = fileURL.deletingLastPathComponent()
        if FileManager.default.fileExists(atPath: directory.path) {
            try FileManager.default.removeItem(at: directory)
        }
    }
}
