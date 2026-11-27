import Foundation
@testable import Primitives
import Testing

@MainActor
struct DevicePurposeStoreTests {
    @Test(arguments: [DevicePurpose.daily, .coldWallet])
    func confirmationPersistsAcrossLaunches(purpose: DevicePurpose) throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let record = directory.appending(path: "purpose")
        let firstLaunch = DevicePurposeStore(recordURL: record)

        #expect(try firstLaunch.read() == nil)
        #expect(!firstLaunch.permitsNetworkAccess)
        #expect(try firstLaunch.confirm(purpose) == purpose)
        let confirmedRecord = try Data(contentsOf: record)

        let nextLaunch = DevicePurposeStore(recordURL: record)
        #expect(try nextLaunch.read() == purpose)
        #expect(try nextLaunch.confirm(.daily) == purpose)
        #expect(nextLaunch.permitsNetworkAccess == (purpose == .daily))
        #expect(try Data(contentsOf: record) == confirmedRecord)
        #expect(try nextLaunch.confirm(purpose == .daily ? .coldWallet : .daily) == purpose)
        #expect(try firstLaunch.read() == purpose)
    }

    @Test
    func corruptRecordKeepsNetworkingDisabled() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let record = directory.appending(path: "purpose")
        let corruptRecord = Data("unexpected-purpose".utf8)
        try corruptRecord.write(to: record)
        let store = DevicePurposeStore(recordURL: record)

        #expect(throws: Error.self) { try store.read() }
        #expect(!store.permitsNetworkAccess)
        #expect(throws: Error.self) { try store.confirm(.daily) }
        #expect(try Data(contentsOf: record) == corruptRecord)
    }

    @Test
    func unavailableStorageKeepsNetworkingDisabled() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let unavailableStores = [DevicePurposeStore(recordURL: nil), DevicePurposeStore(recordURL: directory)]

        for store in unavailableStores {
            #expect(throws: Error.self) { try store.read() }
            #expect(!store.permitsNetworkAccess)
            #expect(throws: Error.self) { try store.confirm(.daily) }
        }
    }

    private func temporaryDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
        return directory
    }
}
