import Foundation
@testable import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Store
import Testing

struct ColdWalletStorageIsolationTests {
    @Test
    func sameWalletHasIndependentPreferences() throws {
        let walletId = UUID().uuidString
        let online = BsvHomeCoreWalletPreferencesStore()
        let cold = BsvHomeCoreWalletPreferencesStore(namespace: "cold_wallet_")
        defer {
            try? online.deletePreferences(walletId: walletId)
            try? cold.deletePreferences(walletId: walletId)
        }

        try online.set(walletId: walletId, key: "selection", value: "online")
        #expect(cold.get(walletId: walletId, key: "selection") == nil)
        try cold.set(walletId: walletId, key: "selection", value: "cold")
        #expect(cold.get(walletId: walletId, key: "selection") == "cold")
        try cold.deletePreferences(walletId: walletId)
        #expect(online.get(walletId: walletId, key: "selection") == "online")
        #expect(cold.get(walletId: walletId, key: "selection") == nil)
    }

    @Test
    func invalidStoragePathsThrow() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("file")
        try Data().write(to: file)

        #expect(throws: (any Error).self) { try DB(fileURL: directory) }
        #expect(throws: (any Error).self) {
            try LocalKeystore(directoryURL: file, keystorePassword: MockKeystorePassword(memoryPassword: ""), environment: .wowPay)
        }
    }
}
