import Foundation
import func BsvHomeCore.bsvColdWalletRole
import enum BsvHomeCore.BsvHomeServiceError
import class BsvHomeCore.BsvHomeWalletService
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
@testable import ManageWallets
import Primitives
import PrimitivesTestKit
@testable import Store
import SwiftUI
import Testing

@MainActor
struct WalletDetailColdPairingTests {
    @Test
    func pairingDelegatesTheFullWalletToItsRootOwnerWithoutCreatingColdStateOrChangingCurrent() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: MockKeystorePassword(), environment: .wowPay, coldWalletStore: store)
        let service = BsvHomeWalletService.mock(keystore: keystore, environment: .wowPay)
        let wallet = publicWallet()
        var opened: [Wallet] = []
        var signingOpened: [Wallet] = []
        var capabilityInputs: [Wallet] = []
        let model = WalletDetailViewModel(navigationPath: .constant(NavigationPath()), wallet: wallet, service: service, preferences: .mock(), coldRole: { try bsvColdWalletRole(wallet: $0.map(), store: store) }, canOpenPairing: {
            capabilityInputs.append($0)
            return true
        }, onOpenPairing: { opened.append($0) }, onOpenSigning: { signingOpened.append($0) })
        #expect(!model.canOpenColdPairing)
        #expect(!model.canOpenColdSigning)
        await model.loadColdPresentation()
        #expect(model.canOpenColdPairing)
        model.onSelectColdPairing()
        #expect(capabilityInputs == [wallet])
        #expect(opened == [wallet])
        #expect(signingOpened.isEmpty)
        #expect(model.canOpenColdSigning)
        model.onSelectColdSigning()
        #expect(signingOpened == [wallet])
        #expect(opened == [wallet])
        #expect(try !store.hasState(walletId: wallet.id.id))
        #expect(service.currentWalletId == nil)
    }

    @Test
    func capabilityErrorsRemainVisibleAndCannotCallThePairingOwner() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: MockKeystorePassword(), environment: .wowPay, coldWalletStore: store)
        let service = BsvHomeWalletService.mock(keystore: keystore, environment: .wowPay)
        var unavailable = true
        var opened = 0
        var signingOpened = 0
        let model = WalletDetailViewModel(navigationPath: .constant(NavigationPath()), wallet: publicWallet(), service: service, preferences: .mock(), coldRole: { try bsvColdWalletRole(wallet: $0.map(), store: store) }, canOpenPairing: { _ in
            if unavailable {
                throw BsvHomeServiceError.Store(msg: "Public metadata unavailable")
            }
            return true
        }, onOpenPairing: { _ in opened += 1 }, onOpenSigning: { _ in signingOpened += 1 })
        await model.loadColdPresentation()
        model.onSelectColdPairing()
        model.onSelectColdSigning()
        #expect(!model.canOpenColdSigning)
        #expect(signingOpened == 0)
        #expect(!model.canOpenColdPairing)
        #expect(model.coldPresentationError != nil)
        #expect(opened == 0)
        unavailable = false
        await model.loadColdPresentation()
        #expect(model.canOpenColdPairing)
        #expect(model.coldPresentationError == nil)
        model.onSelectColdPairing()
        #expect(opened == 1)
        model.onSelectColdSigning()
        #expect(signingOpened == 1)
    }

    @Test
    func aLateCapabilityResultCannotEnablePairingForAChangedWalletSnapshot() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("keys", isDirectory: true), keystorePassword: MockKeystorePassword(), environment: .wowPay, coldWalletStore: store)
        let service = BsvHomeWalletService.mock(keystore: keystore, environment: .wowPay)
        let entered = AsyncStream<Void>.makeStream(bufferingPolicy: .bufferingNewest(1))
        let released = AsyncStream<Bool>.makeStream(bufferingPolicy: .bufferingNewest(1))
        var opened = 0
        var signingOpened = 0
        let model = WalletDetailViewModel(navigationPath: .constant(NavigationPath()), wallet: publicWallet(), service: service, preferences: .mock(), coldRole: { try bsvColdWalletRole(wallet: $0.map(), store: store) }, canOpenPairing: { _ in
            entered.continuation.yield(())
            entered.continuation.finish()
            var release = released.stream.makeAsyncIterator()
            return await release.next() ?? false
        }, onOpenPairing: { _ in opened += 1 }, onOpenSigning: { _ in signingOpened += 1 })
        let load = Task { await model.loadColdPresentation() }
        var starts = entered.stream.makeAsyncIterator()
        try #require(await starts.next() != nil)
        let previous = model.walletQuery.value
        model.walletQuery.value = .mock(id: previous.id, name: "Changed public metadata", type: previous.type, accounts: previous.accounts)
        released.continuation.yield(true)
        released.continuation.finish()
        await load.value
        #expect(!model.canOpenColdPairing)
        model.onSelectColdPairing()
        #expect(opened == 0)
        model.onSelectColdSigning()
        #expect(!model.canOpenColdSigning)
        #expect(signingOpened == 0)
        #expect(model.coldRoleTitle == nil)
    }

    private func publicWallet() -> Wallet {
        .mock(id: .multicoin(address: "1BitcoinEaterAddressDontSendf59kuE"), name: "Public hot HD metadata", type: .multicoin, accounts: [.mock(chain: .bitcoinSv, address: "1BitcoinEaterAddressDontSendf59kuE"), .mock(chain: .ethereum, address: "public-other-chain")])
    }
}
