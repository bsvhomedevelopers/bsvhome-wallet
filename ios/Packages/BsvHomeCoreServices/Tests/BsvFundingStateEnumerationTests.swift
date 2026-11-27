import Foundation
import struct BsvHomeCore.BsvHomeBsvFundingStoredState
@testable import BsvHomeCoreServices
import Store
import StoreTestKit
import Testing

struct BsvFundingStateEnumerationTests {
    @Test
    func includesEveryOriginAndRemovedIdentityWithoutChangingThePayload() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = try BsvHomeCoreBsvMultisigStore(origin: "https://first.invalid", localWalletId: "removed-local-identity", directory: directory)
        let firstPayload = " \n{\"funding\":{},\"opaque\":\"\\u0061\"}\n"
        #expect(try await first.compareAndSwap(expectedRevision: 0, json: firstPayload))
        let firstFile = try #require(try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first)
        #expect(try await first.compareAndSwap(expectedRevision: 1, json: firstPayload))
        let second = try BsvHomeCoreBsvMultisigStore(origin: "https://second.invalid", localWalletId: "other-local-identity", directory: directory)
        let secondPayload = "opaque state interpreted only by Core"
        #expect(try await second.compareAndSwap(expectedRevision: 0, json: secondPayload))
        let secondFile = try #require(try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first { $0 != firstFile })

        let records = try await adapter(directory: directory).fundingStates()
        #expect(records.count == 2)
        #expect(records == records.sorted { $0.identifier < $1.identifier })
        #expect(records.first { $0.identifier == firstFile.deletingPathExtension().lastPathComponent } == BsvHomeBsvFundingStoredState(identifier: firstFile.deletingPathExtension().lastPathComponent, revision: 2, json: firstPayload))
        #expect(records.first { $0.identifier == secondFile.deletingPathExtension().lastPathComponent } == BsvHomeBsvFundingStoredState(identifier: secondFile.deletingPathExtension().lastPathComponent, revision: 1, json: secondPayload))
        #expect(try first.getState()?.json == firstPayload)
        #expect(try await !first.compareAndSwap(expectedRevision: 0, json: "must not replace"))
    }

    @Test(arguments: [false, true])
    func absentAndEmptyDirectoriesReturnNoStatesWithoutCreatingFiles(existing: Bool) async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        if existing {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        #expect(try await adapter(directory: directory).fundingStates().isEmpty)
        #expect(FileManager.default.fileExists(atPath: directory.path) == existing)
    }

    @Test(arguments: [".DS_Store", "orphan.json", String(repeating: "F", count: 64) + ".json", String(repeating: "g", count: 64) + ".json", String(repeating: "a", count: 64) + ".txt"])
    func unknownEntriesFailTheWholeReadAndPreserveKnownData(name: String) async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvMultisigStore(origin: "https://first.invalid", localWalletId: "known", directory: directory)
        #expect(try await store.compareAndSwap(expectedRevision: 0, json: "kept exact"))
        let unknown = directory.appendingPathComponent(name)
        try Data("unrecognized".utf8).write(to: unknown)

        await #expect(throws: BsvMultisigStorageError.invalidEntry) {
            try await adapter(directory: directory).fundingStates()
        }
        #expect(try store.getState()?.json == "kept exact")
        try FileManager.default.removeItem(at: unknown)
        let records = try await adapter(directory: directory).fundingStates()
        #expect(records.count == 1)
        #expect(records.first?.json == "kept exact")
    }

    @Test
    func zeroRevisionRemainsAnErrorWithoutDroppingOtherScopes() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let valid = try BsvHomeCoreBsvMultisigStore(origin: "https://first.invalid", localWalletId: "known", directory: directory)
        #expect(try await valid.compareAndSwap(expectedRevision: 0, json: "retained"))
        let invalid = directory.appendingPathComponent(String(repeating: "0", count: 64) + ".json")
        try JSONSerialization.data(withJSONObject: ["revision": 0, "json": "must not skip"]).write(to: invalid)

        await #expect(throws: BsvMultisigStorageError.invalidRevision) {
            try await adapter(directory: directory).fundingStates()
        }
        #expect(try valid.getState()?.json == "retained")
        try FileManager.default.removeItem(at: invalid)
        #expect(try await adapter(directory: directory).fundingStates().count == 1)
    }

    @Test
    func malformedSnapshotRemainsAnErrorWithoutDroppingOtherScopes() async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let valid = try BsvHomeCoreBsvMultisigStore(origin: "https://first.invalid", localWalletId: "known", directory: directory)
        #expect(try await valid.compareAndSwap(expectedRevision: 0, json: "retained"))
        let invalid = directory.appendingPathComponent(String(repeating: "0", count: 64) + ".json")
        try Data("not a Snapshot".utf8).write(to: invalid)

        await #expect(throws: DecodingError.self) {
            try await adapter(directory: directory).fundingStates()
        }
        #expect(try valid.getState()?.json == "retained")
        try FileManager.default.removeItem(at: invalid)
        #expect(try await adapter(directory: directory).fundingStates().count == 1)
    }

    @Test(arguments: [false, true])
    func onlyRegularFilesAreReadEvenWhenTheFilenameIsValid(symbolicLink: Bool) async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvMultisigStore(origin: "https://first.invalid", localWalletId: "known", directory: directory)
        #expect(try await store.compareAndSwap(expectedRevision: 0, json: "retained"))
        let valid = try #require(try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first)
        let invalid = directory.appendingPathComponent(String(repeating: "0", count: 64) + ".json")
        if symbolicLink {
            try FileManager.default.createSymbolicLink(at: invalid, withDestinationURL: valid)
        } else {
            try FileManager.default.createDirectory(at: invalid, withIntermediateDirectories: true)
        }
        await #expect(throws: BsvMultisigStorageError.invalidEntry) {
            try await adapter(directory: directory).fundingStates()
        }
        #expect(try store.getState()?.json == "retained")
    }

    @Test(arguments: [false, true])
    func aFileOrSymbolicLinkIsNotAnAbsentDirectory(symbolicLink: Bool) async throws {
        let root = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let directory = root.appendingPathComponent("not-the-storage-directory")
        if symbolicLink {
            let target = root.appendingPathComponent("target", isDirectory: true)
            try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
            try FileManager.default.createSymbolicLink(at: directory, withDestinationURL: target)
        } else {
            try Data().write(to: directory)
        }
        await #expect(throws: BsvMultisigStorageError.invalidEntry) {
            try await adapter(directory: directory).fundingStates()
        }
    }

    @Test(arguments: [false, true])
    func unreadableFilesAndDirectoriesAreNeverAnEmptyFundingList(directoryUnreadable: Bool) async throws {
        let directory = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvMultisigStore(origin: "https://first.invalid", localWalletId: "known", directory: directory)
        #expect(try await store.compareAndSwap(expectedRevision: 0, json: "retained"))
        let file = try #require(try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil).first)
        let blocked = directoryUnreadable ? directory : file
        let previous = try #require(FileManager.default.attributesOfItem(atPath: blocked.path)[.posixPermissions])
        defer { try? FileManager.default.setAttributes([.posixPermissions: previous], ofItemAtPath: blocked.path) }
        try FileManager.default.setAttributes([.posixPermissions: 0], ofItemAtPath: blocked.path)

        await #expect(throws: (any Error).self) {
            try await adapter(directory: directory).fundingStates()
        }
    }

    private func adapter(directory: URL) -> BsvHomeCoreBsvAddressStore {
        BsvHomeCoreBsvAddressStore(store: BsvAddressPoolStore(db: DB.mock()), fundingDirectory: directory)
    }

    private func temporaryDirectory() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    }
}
