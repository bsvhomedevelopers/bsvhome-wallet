import CryptoKit
import Foundation
import struct BsvHomeCore.BsvHomeBsvColdWalletStateStamp
import protocol BsvHomeCore.BsvHomeBsvColdWalletStore
import struct BsvHomeCore.BsvHomeBsvColdWalletStoredState
import enum BsvHomeCore.BsvHomeServiceError

public final class BsvHomeCoreBsvColdWalletStore: BsvHomeBsvColdWalletStore, @unchecked Sendable {
    private let directory: URL
    private static let lock = NSLock()
    private static let maximumBytes = 5_242_880

    private struct Snapshot: Codable {
        let revision: UInt64
        let value: String
    }

    public init(directory: URL? = nil) throws {
        guard let directory = directory ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?.appendingPathComponent("WowPayColdWallet", isDirectory: true) else {
            throw BsvColdWalletStorageError.unavailable
        }
        self.directory = directory
    }

    public func hasState(walletId: String) throws -> Bool {
        do {
            return try Self.lock.withLock { try read(walletId: walletId) != nil }
        } catch { throw BsvHomeServiceError.Store(msg: "Cold wallet state could not be read") }
    }

    public func stateStamp(walletId: String) throws -> BsvHomeBsvColdWalletStateStamp? {
        do {
            return try Self.lock.withLock {
                try read(walletId: walletId).map { snapshot in
                    let hash = SHA256.hash(data: Data(snapshot.value.utf8)).map { String(format: "%02x", $0) }.joined()
                    return BsvHomeBsvColdWalletStateStamp(revision: snapshot.revision, payloadHash: hash)
                }
            }
        } catch { throw BsvHomeServiceError.Store(msg: "Cold wallet state could not be read") }
    }

    public func get(walletId: String) async throws -> BsvHomeBsvColdWalletStoredState? {
        do {
            return try Self.lock.withLock {
                try read(walletId: walletId).map { BsvHomeBsvColdWalletStoredState(revision: $0.revision, value: $0.value) }
            }
        } catch { throw BsvHomeServiceError.Store(msg: "Cold wallet state could not be read") }
    }

    public func compareAndSwap(walletId: String, expectedRevision: UInt64?, state: BsvHomeBsvColdWalletStoredState) async throws -> Bool {
        do { return try save(walletId: walletId, expectedRevision: expectedRevision, state: state) }
        catch { throw BsvHomeServiceError.Store(msg: "Cold wallet state could not be saved") }
    }

    private func read(walletId: String) throws -> Snapshot? {
        let url = fileURL(walletId: walletId)
        let data: Data
        do { data = try Data(contentsOf: url, options: [.mappedIfSafe]) }
        catch let error as CocoaError where error.code == .fileReadNoSuchFile || error.code == .fileNoSuchFile { return nil }
        guard data.count <= Self.maximumBytes else { throw BsvColdWalletStorageError.oversized }
        return try JSONDecoder().decode(Snapshot.self, from: data)
    }

    private func save(walletId: String, expectedRevision: UInt64?, state: BsvHomeBsvColdWalletStoredState) throws -> Bool {
        try Self.lock.withLock {
            if let expectedRevision {
                guard expectedRevision < UInt64.max, state.revision == expectedRevision + 1 else { throw BsvColdWalletStorageError.invalidRevision }
            } else if state.revision != 0 {
                throw BsvColdWalletStorageError.invalidRevision
            }
            guard try read(walletId: walletId)?.revision == expectedRevision else { return false }
            let data = try JSONEncoder().encode(Snapshot(revision: state.revision, value: state.value))
            guard data.count <= Self.maximumBytes else { throw BsvColdWalletStorageError.oversized }
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try WalletFileProtection.apply(to: directory)
            let url = fileURL(walletId: walletId)
            try data.write(to: url, options: [.atomic, .completeFileProtection])
            try WalletFileProtection.apply(to: url)
            return true
        }
    }

    private func fileURL(walletId: String) -> URL {
        let name = SHA256.hash(data: Data(walletId.utf8)).map { String(format: "%02x", $0) }.joined()
        return directory.appendingPathComponent(name + ".json")
    }
}
