// Copyright (c). Gem Wallet. All rights reserved.

import CryptoKit
import Foundation
import BsvHomeCore
import BsvHomeCorePrimitives
import struct Primitives.Wallet

public final class BsvMultisigActionScopeFixture: Sendable {
    public let service: BsvHomeBsvMultisigPublicFundingService
    public let payer: BsvHomeCore.Wallet
    public let operationId: String

    private let directory: URL
    private let accesses: Accesses

    public var payerWalletId: String { payer.id }
    public var networkRequestCount: Int { accesses.networkCount }
    public var credentialAccessCount: Int { accesses.credentialCount }

    private init(service: BsvHomeBsvMultisigPublicFundingService, payer: BsvHomeCore.Wallet, operationId: String, directory: URL, accesses: Accesses) {
        self.service = service
        self.payer = payer
        self.operationId = operationId
        self.directory = directory
        self.accesses = accesses
    }

    deinit {
        try? FileManager.default.removeItem(at: directory)
    }

    public static func make(coldStore: (any BsvHomeBsvColdWalletStore)? = nil) async throws -> BsvMultisigActionScopeFixture {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        do {
            guard let url = Bundle.module.url(forResource: "bsv-multisig-public-retry-scope", withExtension: "json") else {
                throw BsvHomeServiceError.Unsupported(msg: "Public action scope fixture resource is unavailable")
            }
            let resource = try JSONDecoder().decode(Resource.self, from: Data(contentsOf: url))
            guard resource.formatVersion == 1, resource.storedRevision > 0 else {
                throw BsvHomeServiceError.Unsupported(msg: "Public action scope fixture envelope is unsupported")
            }
            let wallet = resource.payer.map()
            let wallets = Wallets(wallet: wallet)
            let currentWalletId = "privateKey_bitcoinsv_1BitcoinEaterAddressDontSendf59kuE"
            let session = BsvHomeWalletSessionService.newWithEnvironment(store: Session(walletId: currentWalletId), wallets: wallets, environment: .wowPay)
            let keystore = try BsvHomeKeystore.newWithEnvironment(baseDir: directory.path, environment: .wowPay)
            let accesses = Accesses()
            let provider = Provider(accesses: accesses)
            let password = Password(accesses: accesses)
            let preferenceStore = BsvHomePreferencesStoreMock()
            let preferences = BsvHomePreferencesService(store: preferenceStore)
            let gateway = BsvHomeGateway(provider: provider, preferences: preferenceStore, securePreferences: SecureStore())
            let addressStore = AddressStore(walletId: wallet.id, revision: resource.storedRevision, value: resource.storedValue)
            let addresses = BsvHomeBsvAddressPool(gateway: gateway, walletStore: wallets, store: addressStore, keystore: keystore, password: password)
            let auth = try BsvHomePortalAuthService.newForWallet(
                provider: provider,
                keystore: keystore,
                password: password,
                session: session,
                deviceKey: BsvHomeDeviceKeyService(store: ForbiddenSecureStore(accesses: accesses)),
                store: ForbiddenSecureStore(accesses: accesses),
                baseUrl: resource.origin + "/api/portal",
                language: "en",
                deviceType: "4",
                wallet: wallet,
            )
            let service = try BsvHomeBsvMultisigPublicFundingService(
                session: session,
                gateway: gateway,
                addresses: addresses,
                signer: Signer(accesses: accesses),
                authorization: Authorization(accesses: accesses),
                payerAuth: auth,
                payer: wallet,
                preferences: preferences,
                coldStore: coldStore ?? ColdStore(),
            )
            let results = try await service.fundingResults()
            guard results.contains(where: { $0.operationId == resource.operationId && $0.payerWalletId == wallet.id }) else {
                throw BsvHomeServiceError.Unsupported(msg: "Public action scope fixture journal did not hydrate")
            }
            return BsvMultisigActionScopeFixture(service: service, payer: wallet, operationId: resource.operationId, directory: directory, accesses: accesses)
        } catch {
            try? FileManager.default.removeItem(at: directory)
            throw error
        }
    }

    public func beginRetryFunding() throws -> BsvHomeBsvMultisigActionScope {
        try service.beginAction(intent: .retryPublicFunding(operationId: operationId))
    }

    public func cancel() {
        service.cancel()
    }

    private struct Resource: Decodable, Sendable {
        let formatVersion: Int
        let origin: String
        let payer: Primitives.Wallet
        let operationId: String
        let storedRevision: UInt64
        let storedValue: String
    }

    private final class Accesses: @unchecked Sendable {
        private let lock = NSLock()
        private var network = 0
        private var credentials = 0

        var networkCount: Int { lock.withLock { network } }
        var credentialCount: Int { lock.withLock { credentials } }

        func rejectNetwork() -> AlienError {
            lock.withLock { network += 1 }
            return .RequestError(msg: "Action scope fixture forbids network access")
        }

        func rejectCredentials() -> BsvHomeServiceError {
            lock.withLock { credentials += 1 }
            return .Unsupported(msg: "Action scope fixture forbids credentials and signing")
        }
    }

    private final class Provider: AlienProvider {
        let accesses: Accesses

        init(accesses: Accesses) {
            self.accesses = accesses
        }

        func request(target _: AlienTarget) async throws -> AlienResponse {
            throw accesses.rejectNetwork()
        }
    }

    private final class Password: BsvHomeKeystorePassword {
        let accesses: Accesses

        init(accesses: Accesses) {
            self.accesses = accesses
        }

        func getPassword(createIfMissing _: Bool) throws -> String {
            throw accesses.rejectCredentials()
        }

        func getWalletPassword(walletId _: String) throws -> String? {
            throw accesses.rejectCredentials()
        }

        func deleteWalletPassword(walletId _: String) throws {
            throw accesses.rejectCredentials()
        }

        func authentication() throws -> BsvHomeKeystoreAuthentication {
            throw accesses.rejectCredentials()
        }
    }

    private final class Signer: BsvHomeBsvMultisigSigner {
        let accesses: Accesses

        init(accesses: Accesses) {
            self.accesses = accesses
        }

        func publicKey(wallet _: BsvHomeCore.Wallet) async throws -> String {
            throw accesses.rejectCredentials()
        }

        func authenticate(wallet _: BsvHomeCore.Wallet, scope _: BsvHomeBsvMultisigActionScope?) async throws {
            throw accesses.rejectCredentials()
        }

        func signProposal(wallet _: BsvHomeCore.Wallet, policyJson _: String, proposalJson _: String, recipient _: String, amount _: UInt64, maximumFee _: UInt64, scope _: BsvHomeBsvMultisigActionScope) async throws -> String {
            throw accesses.rejectCredentials()
        }

        func signFunding(wallet _: BsvHomeCore.Wallet, fundingJson _: String, reviewJson _: String, scope _: BsvHomeBsvMultisigActionScope) async throws -> String {
            throw accesses.rejectCredentials()
        }
    }

    private final class Authorization: BsvHomeBsvMultisigAuthorization {
        let accesses: Accesses

        init(accesses: Accesses) {
            self.accesses = accesses
        }

        func authorize(wallet _: BsvHomeCore.Wallet, request _: BsvHomeBsvMultisigAuthorizationRequest, scope _: BsvHomeBsvMultisigActionScope) async throws {
            throw accesses.rejectCredentials()
        }
    }

    private final class Wallets: BsvHomeWalletStore {
        let wallet: BsvHomeCore.Wallet

        init(wallet: BsvHomeCore.Wallet) {
            self.wallet = wallet
        }

        func getWallets() async throws -> [BsvHomeCore.Wallet] {
            [wallet]
        }

        func getWallet(walletId: String) async throws -> BsvHomeCore.Wallet? {
            walletId == wallet.id ? wallet : nil
        }

        func addWallet(wallet _: BsvHomeCore.Wallet) async throws {
            throw unsupported()
        }

        func addWalletWithBsvAddresses(wallet _: BsvHomeCore.Wallet, addresses _: BsvHomeBsvAddressStoredState?) async throws {
            throw unsupported()
        }

        func deleteWallet(walletId _: String) async throws -> Bool {
            throw unsupported()
        }

        func setPinned(walletId _: String, pinned _: Bool) async throws {
            throw unsupported()
        }

        func setName(walletId _: String, name _: String) async throws {
            throw unsupported()
        }

        func setImageUrl(walletId _: String, imageUrl _: String?) async throws {
            throw unsupported()
        }

        private func unsupported() -> BsvHomeServiceError {
            .Unsupported(msg: "Action scope fixture wallet is read-only")
        }
    }

    private final class Session: BsvHomeWalletSessionStore {
        let walletId: String

        init(walletId: String) {
            self.walletId = walletId
        }

        func getCurrentWalletId() throws -> String? {
            walletId
        }

        func setCurrentWalletId(walletId: String?) throws {
            guard walletId == self.walletId else { throw BsvHomeServiceError.Unsupported(msg: "Action scope fixture current wallet is fixed") }
        }
    }

    private final class AddressStore: BsvHomeBsvAddressStore {
        let walletId: String
        let state: BsvHomeBsvAddressStoredState

        init(walletId: String, revision: UInt64, value: String) {
            self.walletId = walletId
            state = BsvHomeBsvAddressStoredState(revision: revision, value: value)
        }

        func fundingStates() async throws -> [BsvHomeBsvFundingStoredState] {
            []
        }

        func get(walletId: String) async throws -> BsvHomeBsvAddressStoredState? {
            walletId == self.walletId ? state : nil
        }

        func save(walletId _: String, expectedRevision _: UInt64?, state _: BsvHomeBsvAddressStoredState) async throws -> Bool {
            throw BsvHomeServiceError.Unsupported(msg: "Action scope fixture address journal is read-only")
        }

        func unresolvedTransactions(walletId _: String) async throws -> [Transaction] {
            []
        }
    }

    final class ColdStore: BsvHomeBsvColdWalletStore, @unchecked Sendable {
        private let lock = NSLock()
        private var values: [String: BsvHomeBsvColdWalletStoredState] = [:]

        func hasState(walletId: String) throws -> Bool {
            lock.withLock { values[walletId] != nil }
        }

        func stateStamp(walletId: String) throws -> BsvHomeBsvColdWalletStateStamp? {
            lock.withLock {
                values[walletId].map {
                    BsvHomeBsvColdWalletStateStamp(revision: $0.revision, payloadHash: SHA256.hash(data: Data($0.value.utf8)).map { String(format: "%02x", $0) }.joined())
                }
            }
        }

        func get(walletId: String) async throws -> BsvHomeBsvColdWalletStoredState? {
            lock.withLock { values[walletId] }
        }

        func compareAndSwap(walletId: String, expectedRevision: UInt64?, state: BsvHomeBsvColdWalletStoredState) async throws -> Bool {
            lock.withLock {
                guard values[walletId]?.revision == expectedRevision else { return false }
                values[walletId] = state
                return true
            }
        }
    }

    private final class ForbiddenSecureStore: BsvHomeSecureStore {
        let accesses: Accesses

        init(accesses: Accesses) {
            self.accesses = accesses
        }

        func get(key _: String) throws -> String? {
            throw accesses.rejectCredentials()
        }

        func set(key _: String, value _: String) throws {
            throw accesses.rejectCredentials()
        }

        func remove(key _: String) throws {
            throw accesses.rejectCredentials()
        }
    }

    private final class SecureStore: BsvHomeSecureStore, @unchecked Sendable {
        private let lock = NSLock()
        private var values: [String: String] = [:]

        func get(key: String) throws -> String? {
            lock.withLock { values[key] }
        }

        func set(key: String, value: String) throws {
            lock.withLock { values[key] = value }
        }

        func remove(key: String) throws {
            lock.withLock { values[key] = nil }
        }
    }
}
