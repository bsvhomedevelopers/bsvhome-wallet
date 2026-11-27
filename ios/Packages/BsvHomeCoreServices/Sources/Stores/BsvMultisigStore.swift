import CryptoKit
import Foundation
import struct BsvHomeCore.BsvHomeBsvFundingStoredState
import protocol BsvHomeCore.BsvHomeBsvMultisigStore
import struct BsvHomeCore.BsvHomeBsvMultisigStoredState

public final class BsvHomeCoreBsvMultisigStore: BsvHomeBsvMultisigStore, @unchecked Sendable {
    private struct Snapshot: Codable {
        let revision: UInt64
        let json: String
    }

    private let directory: URL
    private let fileURL: URL
    private static let lock = NSLock()

    public init(origin: String, localWalletId: String, directory: URL? = nil) throws {
        let directory = try Self.storageDirectory(directory)
        self.directory = directory
        let scope = Data((origin + "|" + localWalletId).utf8)
        let identifier = SHA256.hash(data: scope).map { String(format: "%02x", $0) }.joined()
        fileURL = directory.appendingPathComponent(identifier + ".json")
    }

    public func getState() throws -> BsvHomeBsvMultisigStoredState? {
        try Self.lock.withLock {
            try read().map { BsvHomeBsvMultisigStoredState(revision: $0.revision, json: $0.json) }
        }
    }

    public func compareAndSwap(expectedRevision: UInt64, json: String) async throws -> Bool {
        try save(expectedRevision: expectedRevision, json: json)
    }

    static func fundingStates(directory: URL? = nil) throws -> [BsvHomeBsvFundingStoredState] {
        try lock.withLock {
            let directory = try storageDirectory(directory)
            let attributes: [FileAttributeKey: Any]
            do { attributes = try FileManager.default.attributesOfItem(atPath: directory.path) }
            catch let error as CocoaError where error.code == .fileReadNoSuchFile || error.code == .fileNoSuchFile { return [] }
            guard attributes[.type] as? FileAttributeType == .typeDirectory else { throw BsvMultisigStorageError.invalidEntry }
            let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            return try files.sorted { $0.lastPathComponent < $1.lastPathComponent }.map { file in
                let name = file.lastPathComponent
                guard name.hasSuffix(".json") else { throw BsvMultisigStorageError.invalidEntry }
                let identifier = String(name.dropLast(5))
                guard identifier.utf8.count == 64,
                      identifier.utf8.allSatisfy({ ($0 >= 48 && $0 <= 57) || ($0 >= 97 && $0 <= 102) }),
                      try FileManager.default.attributesOfItem(atPath: file.path)[.type] as? FileAttributeType == .typeRegular
                else { throw BsvMultisigStorageError.invalidEntry }
                let snapshot = try read(fileURL: file)
                return BsvHomeBsvFundingStoredState(identifier: identifier, revision: snapshot.revision, json: snapshot.json)
            }
        }
    }

    private static func storageDirectory(_ directory: URL?) throws -> URL {
        guard let directory = directory ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?.appendingPathComponent("WowPayMultisig", isDirectory: true) else {
            throw BsvMultisigStorageError.unavailable
        }
        return directory
    }

    private static func read(fileURL: URL) throws -> Snapshot {
        let snapshot = try JSONDecoder().decode(Snapshot.self, from: Data(contentsOf: fileURL))
        guard snapshot.revision > 0 else { throw BsvMultisigStorageError.invalidRevision }
        return snapshot
    }

    private func read() throws -> Snapshot? {
        do { return try Self.read(fileURL: fileURL) }
        catch let error as CocoaError where error.code == .fileReadNoSuchFile || error.code == .fileNoSuchFile { return nil }
    }

    private func save(expectedRevision: UInt64, json: String) throws -> Bool {
        try Self.lock.withLock {
            guard expectedRevision < UInt64.max else { throw BsvMultisigStorageError.invalidRevision }
            guard try (read()?.revision ?? 0) == expectedRevision else { return false }
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            #if os(iOS)
                try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: directory.path)
            #endif
            var protectedDirectory = directory
            var values = URLResourceValues()
            values.isExcludedFromBackup = true
            try protectedDirectory.setResourceValues(values)
            let data = try JSONEncoder().encode(Snapshot(revision: expectedRevision + 1, json: json))
            try data.write(to: fileURL, options: [.atomic, .completeFileProtection])
            #if os(iOS)
                try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: fileURL.path)
            #endif
            var protectedFile = fileURL
            try protectedFile.setResourceValues(values)
            return true
        }
    }
}
