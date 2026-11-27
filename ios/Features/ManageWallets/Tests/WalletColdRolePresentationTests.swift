import Foundation
import func BsvHomeCore.bsvColdWalletRole
import enum BsvHomeCore.BsvHomeServiceError
import class BsvHomeCore.BsvHomeWalletService
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
@testable import ManageWallets
import Primitives
import PrimitivesTestKit
@testable import Store
import SwiftUI
import Testing

@MainActor
struct WalletColdRolePresentationTests {
    @Test
    func signersAreGroupedSeparatelyWhileObserversRemainPersonalWithoutChangingStoredWallets() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: MockKeystorePassword(), environment: .wowPay, coldWalletStore: store)
        let service = BsvHomeWalletService.mock(keystore: keystore, environment: .wowPay)
        let hot = wallet("hot", type: .single)
        let observer = wallet("observer", type: .view)
        let signer = wallet("signer", type: .single)
        let pinnedSigner = wallet("pinned-signer", type: .single)
        for wallet in [observer, signer, pinnedSigner] {
            #expect(try await store.compareAndSwap(walletId: wallet.id.id, expectedRevision: nil, state: .init(revision: 0, value: "public-routing-marker")))
        }
        let model = WalletsSceneViewModel(navigationPath: .constant(NavigationPath()), walletService: service, preferences: .mock(), isPresentingCreateWalletSheet: .constant(false), isPresentingImportWalletSheet: .constant(false), coldRole: { try bsvColdWalletRole(wallet: $0.map(), store: store) })
        let rawWallets = [hot, observer, signer]
        model.walletsQuery.value = rawWallets
        model.pinnedWalletsQuery.value = [pinnedSigner]
        #expect(!model.coldRolesReady)
        #expect(model.personalWallets.isEmpty)
        model.refreshColdRoles()
        #expect(model.coldRolesReady)
        #expect(Set(model.personalWallets.map(\.id)) == Set([hot.id, observer.id]))
        #expect(model.personalPinnedWallets.isEmpty)
        #expect(model.coldSignerWallets.map(\.id).first == pinnedSigner.id)
        #expect(Set(model.coldSignerWallets.map(\.id)) == Set([signer.id, pinnedSigner.id]))
        #expect(model.coldRole(for: observer) == .observer)
        #expect(model.walletsQuery.value == rawWallets)
        #expect(model.pinnedWalletsQuery.value == [pinnedSigner])
        #expect(try await store.get(walletId: signer.id.id)?.value == "public-routing-marker")
        #expect(service.currentWalletId == nil)
    }

    @Test
    func aRealRoleReadFailureDisablesSelectionUntilRetryAndNeverClassifiesUnknownAsHot() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let coldDirectory = directory.appendingPathComponent("cold", isDirectory: true)
        let store = try BsvHomeCoreBsvColdWalletStore(directory: coldDirectory)
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: MockKeystorePassword(), environment: .wowPay, coldWalletStore: store)
        let service = BsvHomeWalletService.mock(keystore: keystore, environment: .wowPay)
        let signer = wallet("signer", type: .single)
        #expect(try await store.compareAndSwap(walletId: signer.id.id, expectedRevision: nil, state: .init(revision: 0, value: "public-routing-marker")))
        let file = try #require(FileManager.default.contentsOfDirectory(at: coldDirectory, includingPropertiesForKeys: nil).first)
        let original = try Data(contentsOf: file)
        let model = WalletsSceneViewModel(navigationPath: .constant(NavigationPath()), walletService: service, preferences: .mock(), isPresentingCreateWalletSheet: .constant(false), isPresentingImportWalletSheet: .constant(false), coldRole: { try bsvColdWalletRole(wallet: $0.map(), store: store) })
        model.walletsQuery.value = [signer]
        model.refreshColdRoles()
        #expect(model.coldRolesReady)
        try Data("{".utf8).write(to: file, options: .atomic)
        model.refreshColdRoles()
        var selections = 0
        model.onSelect(wallet: signer) { selections += 1 }
        #expect(selections == 0)
        #expect(service.currentWalletId == nil)
        #expect(!model.coldRolesReady)
        #expect(model.coldRole(for: signer) == nil)
        #expect(model.personalWallets.isEmpty)
        #expect(model.coldSignerWallets.isEmpty)
        #expect(model.coldRoleError != nil)
        #expect(try Data(contentsOf: file) == Data("{".utf8))
        try original.write(to: file, options: .atomic)
        model.refreshColdRoles()
        #expect(model.coldRolesReady)
        #expect(model.coldRoleError == nil)
        #expect(model.coldSignerWallets.map(\.id) == [signer.id])
    }

    @Test
    func changingThePublicWalletSnapshotInvalidatesItsPreviousClassification() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: MockKeystorePassword(), environment: .wowPay, coldWalletStore: store)
        let service = BsvHomeWalletService.mock(keystore: keystore, environment: .wowPay)
        let first = wallet("first", type: .single)
        let second = wallet("second", type: .view)
        let model = WalletsSceneViewModel(navigationPath: .constant(NavigationPath()), walletService: service, preferences: .mock(), isPresentingCreateWalletSheet: .constant(false), isPresentingImportWalletSheet: .constant(false), coldRole: { try bsvColdWalletRole(wallet: $0.map(), store: store) })
        model.walletsQuery.value = [first]
        model.refreshColdRoles()
        #expect(model.coldRolesReady)
        model.walletsQuery.value = [second]
        #expect(!model.coldRolesReady)
        #expect(model.personalWallets.isEmpty)
        #expect(model.coldRole(for: first) == nil)
        model.refreshColdRoles()
        #expect(model.coldRolesReady)
        #expect(model.personalWallets.map(\.id) == [second.id])
    }

    private func wallet(_ name: String, type: WalletType) -> Wallet {
        let address = switch name {
        case "hot", "first": "1BitcoinEaterAddressDontSendf59kuE"
        case "observer", "second": "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5"
        case "signer": "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD"
        default: "1GKaM5VVHt2PNLyFjRWbRBwCAx1n6pqUkQ"
        }
        let id: WalletId = type == .view ? .view(chain: .bitcoinSv, address: address) : .single(chain: .bitcoinSv, address: address)
        return .mock(id: id, name: name, type: type, accounts: [.mock(chain: .bitcoinSv, address: address)])
    }
}
