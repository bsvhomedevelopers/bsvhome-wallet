import class BsvHomeCore.BsvHomeBsvAddressPool
import class BsvHomeCore.BsvHomeWalletService
import BsvHomeCorePrimitives
@testable import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import Store
import StoreTestKit
import Testing

struct BsvAddressPoolTests {
    @Test
    func phraseImportStoresTenAddressesAndOneStableAccount() async throws {
        let db = DB.mockWithChains([.bitcoinSv, .tron])
        let keystore = LocalKeystore.mock()
        defer { destroyFixture(keystore) }
        let service = BsvHomeWalletService.mock(keystore: keystore, db: db)
        let wallet = try await service.importWallet(name: "BSV", type: .singlePhrase(words: LocalKeystore.words, chain: Chain.bitcoinSv.map()), source: .import).wallet
        let pool = makePool(db: db, keystore: keystore)
        let snapshot = try #require(try await pool.snapshot(walletId: wallet.id.id))
        let account = try #require(wallet.accounts.first(where: { $0.chain == .bitcoinSv }))

        #expect(snapshot.records.count == 10)
        #expect(Set(snapshot.records.map(\.address)).count == 10)
        #expect(snapshot.records.filter(\.isMain).map(\.address) == [account.address])
        #expect(snapshot.currentAddress == account.address)
        #expect(try BsvAddressPoolStore(db: db).get(walletId: wallet.id)?.revision == 0)
        #expect(try WalletStore(db: db).getWallet(id: wallet.id)?.accounts.filter { $0.chain == .bitcoinSv }.count == 1)

        _ = try await service.importWallet(name: "Same seed", type: .singlePhrase(words: LocalKeystore.words, chain: Chain.bitcoinSv.map()), source: .import)
        #expect(try await pool.snapshot(walletId: wallet.id.id) == snapshot)
    }

    @Test
    func historicalWalletCreatesPoolOnlyAtExplicitFirstUse() async throws {
        let db = DB.mockWithChains([.bitcoinSv])
        let passwords = MockKeystorePassword()
        let keystore = LocalKeystore.mock(keystorePassword: passwords)
        defer { destroyFixture(keystore) }
        let wallet = try keystore.importWallet(name: "Historical", type: .singlePhrase(words: LocalKeystore.words, chain: Chain.bitcoinSv.map()))
        try WalletStore(db: db).addWallet(wallet)
        let pool = makePool(db: db, keystore: keystore)
        let reads = passwords.getPasswordCallsCount

        #expect(try await pool.snapshot(walletId: wallet.id.id) == nil)
        #expect(passwords.getPasswordCallsCount == reads)
        let prepared = try await pool.prepare(walletId: wallet.id.id)
        #expect(passwords.getPasswordCallsCount == reads + 1)
        #expect(prepared.records.count == 10)
        #expect(prepared.currentAddress == wallet.accounts.first?.address)
        #expect(try WalletStore(db: db).getWallet(id: wallet.id)?.accounts == wallet.accounts)

        let reopened = makePool(db: db, keystore: keystore)
        #expect(try await reopened.prepare(walletId: wallet.id.id) == prepared)
        #expect(passwords.getPasswordCallsCount == reads + 1)
    }

    @Test
    func watchedWalletKeepsSingleAddressWithoutPasswordRead() async throws {
        let db = DB.mockWithChains([.bitcoinSv])
        let passwords = MockKeystorePassword()
        let keystore = LocalKeystore.mock(keystorePassword: passwords)
        defer { destroyFixture(keystore) }
        let wallet = try keystore.importWallet(name: "Watch", type: .address(address: "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA", chain: Chain.bitcoinSv.map()))
        try WalletStore(db: db).addWallet(wallet)

        let snapshot = try await makePool(db: db, keystore: keystore).prepare(walletId: wallet.id.id)
        #expect(snapshot.records.count == 1)
        #expect(snapshot.currentAddress == wallet.accounts.first?.address)
        #expect(passwords.getPasswordCallsCount == 0)
        #expect(try BsvAddressPoolStore(db: db).get(walletId: wallet.id) == nil)
    }

    private func makePool(db: DB, keystore: LocalKeystore) -> BsvHomeBsvAddressPool {
        GatewayService.mock().bsvAddressPool(
            walletStore: BsvHomeCoreWalletStore(store: WalletStore(db: db)),
            store: BsvHomeCoreBsvAddressStore(store: BsvAddressPoolStore(db: db)),
            keystore: keystore.bsvHomeKeystore,
            password: BsvHomeCoreKeystorePassword(keystore: keystore),
        )
    }

    private func destroyFixture(_ keystore: LocalKeystore) {
        do {
            try keystore.destroy()
        } catch {
            Issue.record(error)
        }
    }
}
