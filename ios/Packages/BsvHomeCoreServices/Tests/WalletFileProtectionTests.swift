import Foundation
import BsvHomeCoreServices
import Testing

struct WalletFileProtectionTests {
    @Test
    func backupExclusionIsReappliedAfterAtomicReplacement() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let nested = root.appendingPathComponent("Keystore", isDirectory: true)
        try FileManager.default.createDirectory(at: nested, withIntermediateDirectories: true)
        let paths = [nested.appendingPathComponent("fixture"), root.appendingPathComponent("offline.sqlite"), root.appendingPathComponent("offline.sqlite-wal"), root.appendingPathComponent("offline.sqlite-shm")]
        for path in paths { try Data("public fixture".utf8).write(to: path, options: .atomic) }
        try WalletFileProtection.apply(to: root)
        for path in paths { try Data("replacement fixture".utf8).write(to: path, options: .atomic) }
        try WalletFileProtection.apply(to: root)
        for path in [root, nested] + paths {
            #expect(try path.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == true)
            #if os(iOS) && !targetEnvironment(simulator)
                let value = try FileManager.default.attributesOfItem(atPath: path.path)[.protectionKey]
                let protection = (value as? FileProtectionType)?.rawValue ?? value as? String
                #expect(protection == FileProtectionType.complete.rawValue)
            #endif
        }
    }

    @Test
    func symbolicLinksCannotRedirectProtectionOutsideTheDirectory() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(at: root.appendingPathComponent("link"), withDestinationURL: root.deletingLastPathComponent())
        #expect(throws: (any Error).self) { try WalletFileProtection.apply(to: root) }
    }
}
