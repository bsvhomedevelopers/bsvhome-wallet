import Foundation
@testable import BsvHome
import BsvHomeCore
import BsvHomeCorePrimitives
import Primitives
import Testing

@MainActor
struct AlignmentRegressionWalletTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_QA_WALLET"] == "1", "Requires explicit local QA wallet provisioning opt-in"))
    func provisionOneEncryptedIdentityInTheBaseAppContainer() async {
        do {
            try validateHost()
            let markerURL = try markerLocation()
            let marker = try readMarker(at: markerURL)
            let resolver = AppResolver.main
            let service = resolver.services.viewModelFactory.walletService
            let store = resolver.storages.storeManager.walletStore
            let before = try store.getWallets()
            try validateWallets(before, marker: marker, currentWalletId: service.currentWalletId())

            if let marker {
                guard let existing = before.first(where: { $0.id.id == marker.walletId }) else { throw Failure.rejected }
                let hasStoredSecret = resolver.storages.keystore.bsvHomeKeystore.exists(keystoreId: resolver.storages.keystore.bsvHomeKeystore.keystoreId(walletId: existing.id.id))
                guard hasStoredSecret, service.isMultisigIdentity(walletId: existing.id.id), !Task.isCancelled else { throw Failure.rejected }
                try service.setCurrentWalletId(walletId: existing.id.id)
                let reusedExistingIdentity = try service.currentWalletId() == existing.id.id
                #expect(reusedExistingIdentity)
                #expect(try store.getWallets().count == before.count)
                return
            }

            guard before.isEmpty, !Task.isCancelled else { throw Failure.rejected }
            let result = try await createIdentity(using: service)
            guard !Task.isCancelled else { throw Failure.rejected }
            let created = try service.completeMultisigIdentityImport(result: result, source: .create).map()
            guard service.isMultisigIdentity(walletId: created.id.id) else { throw Failure.rejected }
            let publicMarker = try Marker(identifier: Self.identifier, walletId: created.id.id, address: signingAddress(created))
            let after = try store.getWallets()
            guard after.count == before.count + 1, Set(before.map(\.id.id)).isSubset(of: Set(after.map(\.id.id))) else { throw Failure.rejected }
            try validateWallets(after, marker: publicMarker, currentWalletId: service.currentWalletId())
            try saveMarker(publicMarker, at: markerURL)
            guard !Task.isCancelled else { throw Failure.rejected }
            let selectedCreatedIdentity = try service.currentWalletId() == created.id.id
            let oneQaIdentity = after.filter { $0.id.id == publicMarker.walletId }.count == 1
            #expect(selectedCreatedIdentity)
            #expect(oneQaIdentity)
        } catch {
            Issue.record("Local QA identity provisioning was rejected or failed; wallet material is not included in diagnostics")
        }
    }

    private static let simulatorId = "A4BB51ED-D5E8-433D-8280-6CC2D8CE832F"
    private static let identifier = "bsvhome-alignment-local-qa-v1"
    private static let walletName = "Alignment QA Identity"
    private static let watchAddresses: Set<String> = ["1BitcoinEaterAddressDontSendf59kuE", "1CounterpartyXXXXXXXXXXXXXXXUWLpVr"]

    private func validateHost() throws {
        #if DEBUG && targetEnvironment(simulator)
            guard BsvHomeAppHostMode.isUnitTestHost,
                  ProcessInfo.processInfo.environment["SIMULATOR_UDID"]?.uppercased() == Self.simulatorId,
                  Bundle.main.bundleIdentifier == "com.bsvhome.ios",
                  Bundle.main.bundleURL.lastPathComponent == "BsvHome.app",
                  Bundle.main.executableURL?.lastPathComponent == "BsvHome"
            else { throw Failure.rejected }
            let home = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true).resolvingSymlinksInPath()
            let tail = Array(home.pathComponents.suffix(7))
            guard tail.count == 7,
                  Array(tail.prefix(6)) == ["Devices", Self.simulatorId, "data", "Containers", "Data", "Application"],
                  UUID(uuidString: tail[6]) != nil
            else { throw Failure.rejected }
        #else
            throw Failure.rejected
        #endif
    }

    private func markerLocation() throws -> URL {
        let home = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true).resolvingSymlinksInPath()
        guard let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first,
              support.resolvingSymlinksInPath().path.hasPrefix(home.path + "/")
        else { throw Failure.rejected }
        return support.appendingPathComponent("AlignmentQaIdentity", isDirectory: true).appendingPathComponent("public-marker.json")
    }

    private func readMarker(at url: URL) throws -> Marker? {
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        let marker = try JSONDecoder().decode(Marker.self, from: Data(contentsOf: url))
        guard marker.identifier == Self.identifier, !marker.walletId.isEmpty, !marker.address.isEmpty else { throw Failure.rejected }
        return marker
    }

    private func saveMarker(_ marker: Marker, at url: URL) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try JSONEncoder().encode(marker).write(to: url, options: [.atomic, .completeFileProtection])
    }

    private func validateWallets(_ wallets: [Primitives.Wallet], marker: Marker?, currentWalletId: String?) throws {
        guard wallets.count <= Self.watchAddresses.count + (marker == nil ? 0 : 1) else { throw Failure.rejected }
        var watchAddresses = Set<String>()
        var qaCount = 0
        for wallet in wallets {
            let address = try signingAddress(wallet)
            if let marker, wallet.id.id == marker.walletId {
                guard wallet.name == Self.walletName, wallet.type == .multicoin, wallet.source == .create, address == marker.address else { throw Failure.rejected }
                qaCount += 1
            } else {
                guard wallet.type == .view, wallet.source == .import, Self.watchAddresses.contains(address), watchAddresses.insert(address).inserted else { throw Failure.rejected }
            }
        }
        guard qaCount == (marker == nil ? 0 : 1) else { throw Failure.rejected }
        if let currentWalletId {
            guard wallets.contains(where: { $0.id.id == currentWalletId }) else { throw Failure.rejected }
        }
    }

    private func signingAddress(_ wallet: Primitives.Wallet) throws -> String {
        guard wallet.accounts.count == 1, let account = wallet.accounts.first, account.chain == .bitcoinSv else { throw Failure.rejected }
        return account.address
    }

    private func createIdentity(using service: BsvHomeWalletService) async throws -> BsvHomeWalletImportResult {
        var words = try service.createWallet()
        defer { words.removeAll(keepingCapacity: false) }
        guard words.count == 12, !Task.isCancelled else { throw Failure.rejected }
        let result = try await service.importWallet(name: Self.walletName, import: .multicoinPhrase(words: words, chains: [Primitives.Chain.bitcoinSv.rawValue]), source: .create)
        guard case .new = result else { throw Failure.rejected }
        return result
    }

    private struct Marker: Codable {
        let identifier: String
        let walletId: String
        let address: String
    }

    private enum Failure: Error {
        case rejected
    }
}
