import Foundation
import class BsvHomeCore.BsvHomeBsvAddressPool
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import Store
import StoreTestKit
import Testing

@Suite(.serialized)
struct BsvAddressCancellationTests {
    @Test
    func realBridgeCancelsSuspendedRPCAndPreservesProgressForRetry() async throws {
        let db = DB.mockWithChains([.bitcoinSv])
        let keystore = LocalKeystore.mock()
        defer { destroyFixture(keystore) }
        let wallet = try keystore.importWallet(name: "Cancel fixture", type: .singlePhrase(words: LocalKeystore.words, chain: Chain.bitcoinSv.map()))
        try WalletStore(db: db).addWallet(wallet)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [BsvCancellationURLProtocol.self]
        configuration.timeoutIntervalForRequest = 5
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        BsvCancellationURLProtocol.state.reset(suspendAfter: 11)
        let pool = makePool(db: db, keystore: keystore, gateway: .mock(session: session))
        let initial = try await pool.prepare(walletId: wallet.id.id)
        let generation = try pool.scanGeneration(walletId: wallet.id.id)
        let scan = Task { try await pool.syncAtGeneration(walletId: wallet.id.id, generation: generation, extend: false) }
        try await waitFor { BsvCancellationURLProtocol.state.requestCount >= 12 }

        try pool.cancelScan(walletId: wallet.id.id)
        try await waitFor(attempts: 200) { BsvCancellationURLProtocol.state.stoppedCount > 0 }
        do {
            _ = try await scan.value
            Issue.record("Cancelled Core scan completed successfully")
        } catch {
            #expect(error.isCancelled)
        }
        let interrupted = try #require(try await pool.snapshot(walletId: wallet.id.id))
        #expect(interrupted.currentAddress == initial.currentAddress)
        #expect(interrupted.receiveScanned == 1)
        #expect(interrupted.changeScanned == 0)
        #expect(interrupted.discovery != .complete)

        BsvCancellationURLProtocol.state.allowResponses()
        let resumed = try await pool.sync(walletId: wallet.id.id)
        #expect(resumed.discovery == .complete)
        #expect(resumed.receiveScanned == 20)
        #expect(resumed.changeScanned == 20)
    }

    @Test
    func generationCapturedBeforeCancellationCannotStartAtFirstRustPoll() async throws {
        let db = DB.mockWithChains([.bitcoinSv])
        let passwords = MockKeystorePassword()
        let keystore = LocalKeystore.mock(keystorePassword: passwords)
        defer { destroyFixture(keystore) }
        let wallet = try keystore.importWallet(name: "Before poll", type: .singlePhrase(words: LocalKeystore.words, chain: Chain.bitcoinSv.map()))
        try WalletStore(db: db).addWallet(wallet)
        let pool = makePool(db: db, keystore: keystore, gateway: .mock())
        let reads = passwords.getPasswordCallsCount
        let generation = try pool.scanGeneration(walletId: wallet.id.id)
        try pool.cancelScan(walletId: wallet.id.id)

        do {
            _ = try await pool.syncAtGeneration(walletId: wallet.id.id, generation: generation, extend: false)
            Issue.record("An obsolete generation started scanning")
        } catch {
            #expect(error.isCancelled)
        }
        #expect(passwords.getPasswordCallsCount == reads)
        #expect(try BsvAddressPoolStore(db: db).get(walletId: wallet.id) == nil)
        #expect(try await pool.prepare(walletId: wallet.id.id).records.count == 10)
    }

    private func makePool(db: DB, keystore: LocalKeystore, gateway: GatewayService) -> BsvHomeBsvAddressPool {
        gateway.bsvAddressPool(
            walletStore: BsvHomeCoreWalletStore(store: WalletStore(db: db)),
            store: BsvHomeCoreBsvAddressStore(store: BsvAddressPoolStore(db: db)),
            keystore: keystore.bsvHomeKeystore,
            password: BsvHomeCoreKeystorePassword(keystore: keystore),
        )
    }

    private func waitFor(attempts: Int = 3000, _ condition: () -> Bool) async throws {
        for _ in 0 ..< attempts {
            if condition() {
                return
            }
            try await Task.sleep(for: .milliseconds(10))
        }
        throw AnyError("Timed out waiting for the suspended BSV request")
    }

    private func destroyFixture(_ keystore: LocalKeystore) {
        do {
            try keystore.destroy()
        } catch {
            Issue.record(error)
        }
    }
}
