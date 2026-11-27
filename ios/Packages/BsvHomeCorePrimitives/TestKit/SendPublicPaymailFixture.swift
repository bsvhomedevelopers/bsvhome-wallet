import BigInt
import Foundation
import BsvHomeCore
import enum Primitives.Chain
import enum Primitives.WalletId

@MainActor
public final class SendPublicPaymailFixture {
    public enum Output: Sendable {
        case p2pkh
        case p2ms
    }

    public static let handle = "receiver@example.invalid"
    public let service: BsvHomeSendService
    public let asset: BsvHomeCore.Asset
    public let wallet: BsvHomeCore.Wallet

    private let directory: URL
    private let provider: Provider

    public var managementRequestCount: Int { provider.managementRequestCount }
    public var publicRequestCount: Int { provider.publicRequestCount }
    public var credentialAccessCount: Int { provider.credentialAccessCount }
    public var rejectedRequestCount: Int { provider.rejectedRequestCount }

    private init(service: BsvHomeSendService, asset: BsvHomeCore.Asset, wallet: BsvHomeCore.Wallet, directory: URL, provider: Provider) {
        self.service = service
        self.asset = asset
        self.wallet = wallet
        self.directory = directory
        self.provider = provider
    }

    deinit {
        try? FileManager.default.removeItem(at: directory)
    }

    public static func make(output: Output = .p2ms) async throws -> SendPublicPaymailFixture {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            guard let resource = Bundle.module.url(forResource: "bsv-multisig-public-proposal", withExtension: "json"),
                  let object = try JSONSerialization.jsonObject(with: Data(contentsOf: resource)) as? [String: Any],
                  let proposal = object["proposal"] as? [String: Any],
                  let policy = proposal["policy"] as? [String: Any],
                  let publicKeys = policy["publicKeys"] as? [String],
                  publicKeys.count >= 2
            else { throw BsvHomeServiceError.Unsupported(msg: "Public Paymail fixture resource is unavailable") }
            let keys = Array(publicKeys.prefix(2))
            let keystore = try BsvHomeKeystore.newWithEnvironment(baseDir: directory.path, environment: .wowPay)
            let address = try keystore.bsvMultisigMemberAddress(publicKey: keys[0])
            let descriptor = try keystore.bsvMultisigPolicy(threshold: 2, publicKeys: keys)
            guard let data = descriptor.data(using: .utf8),
                  let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let lockingScript = object["lockingScript"] as? String
            else { throw BsvHomeServiceError.Unsupported(msg: "Public Paymail fixture policy is unavailable") }
            let walletId = Primitives.WalletId.privateKey(chain: .bitcoinSv, address: address).id
            let wallet = BsvHomeCore.Wallet(
                id: walletId,
                externalId: nil,
                name: "Public Paymail fixture",
                index: 0,
                walletType: .privateKey,
                accounts: [BsvHomeCore.Account(chain: Primitives.Chain.bitcoinSv.rawValue, address: address, derivationPath: "", extendedPublicKey: nil)],
                isPinned: false,
                imageUrl: nil,
                source: .import,
            )
            let provider = Provider(address: address, identityPublicKey: keys[1], lockingScript: lockingScript, output: output)
            let wallets = Wallets(wallet: wallet, provider: provider)
            let session = BsvHomeWalletSessionService.newWithEnvironment(store: Session(walletId: walletId, provider: provider), wallets: wallets, environment: .wowPay)
            let secure = SecureStore()
            try secure.set(key: "session", value: String(decoding: JSONSerialization.data(withJSONObject: [
                "base_url": Provider.baseUrl,
                "wallet_id": walletId,
                "address": address,
                "token": UUID().uuidString,
                "expires_at": NSNull(),
            ]), as: UTF8.self))
            let auth = try BsvHomePortalAuthService(
                provider: provider,
                keystore: keystore,
                password: Password(provider: provider),
                session: session,
                deviceKey: BsvHomeDeviceKeyService(store: CredentialStore(provider: provider)),
                store: secure,
                baseUrl: Provider.baseUrl,
                language: "en",
                deviceType: "4",
            )
            let sender = try BsvHomePaymailService(auth: auth, trustedPublicOrigins: [Provider.origin], walletId: walletId)
            let resolver = BsvHomePaymailDestinationService.newForWallet(provider: provider, sender: sender)
            let preferencesStore = BsvHomePreferencesStoreMock()
            let preferences = BsvHomePreferencesService(store: preferencesStore)
            let gateway = BsvHomeGateway.newWithEnvironment(provider: provider, preferences: preferencesStore, securePreferences: SecureStore(), environment: .wowPay)
            let addresses = BsvHomeAddressStoreMock()
            let names = BsvHomeNameService(api: BsvHomeDeviceApiClient(provider: provider, deviceKey: BsvHomeDeviceKeyService(store: CredentialStore(provider: provider))), store: addresses)
            let stake = BsvHomeStakeService(
                gateway: gateway,
                staticApi: BsvHomeStaticApiClient(provider: provider),
                store: StakeStore(provider: provider),
                addressStore: addresses,
                explorer: BsvHomeExplorerService(preferences: preferences),
                preferences: preferences,
                session: session,
            )
            let service = BsvHomeSendService(
                recipient: BsvHomeRecipientService(names: names, payments: BsvHomePaymentService(provider: provider), session: session),
                amount: BsvHomeAmountService(stake: stake, preferences: preferences, session: session),
                paymail: resolver,
            )
            let asset = BsvHomeCore.Asset(id: Primitives.Chain.bitcoinSv.rawValue, name: "Bitcoin SV", symbol: "BSV", decimals: 8, assetType: .native)
            return SendPublicPaymailFixture(service: service, asset: asset, wallet: wallet, directory: directory, provider: provider)
        } catch {
            try? FileManager.default.removeItem(at: directory)
            throw error
        }
    }

    public func destination(value: BigInt, useMaxAmount: Bool = false) async throws -> BsvHomeSendDestination {
        try await service.resolveDestination(asset: asset, input: Self.handle, state: .none, name: nil, value: value, useMaxAmount: useMaxAmount)
    }

    public func prepare(value: BigInt, useMaxAmount: Bool = false) async throws -> BsvHomeSendPublicFundingPreparation {
        switch try await destination(value: value, useMaxAmount: useMaxAmount) {
        case let .publicFunding(preparation): preparation
        case .transfer: throw BsvHomeServiceError.Unsupported(msg: "Public Paymail fixture expected P2MS output")
        }
    }

    public func setOutput(_ output: Output) {
        provider.setOutput(output)
    }

    private final class Provider: AlienProvider, @unchecked Sendable {
        static let origin = "https://sender.invalid"
        static let baseUrl = origin + "/api/portal"
        private let lock = NSLock()
        private let address: String
        private let identityPublicKey: String
        private let lockingScript: String
        private var output: Output
        private var managementRequests = 0
        private var publicRequests = 0
        private var credentials = 0
        private var rejectedRequests = 0

        var managementRequestCount: Int { lock.withLock { managementRequests } }
        var publicRequestCount: Int { lock.withLock { publicRequests } }
        var credentialAccessCount: Int { lock.withLock { credentials } }
        var rejectedRequestCount: Int { lock.withLock { rejectedRequests } }

        init(address: String, identityPublicKey: String, lockingScript: String, output: Output) {
            self.address = address
            self.identityPublicKey = identityPublicKey
            self.lockingScript = lockingScript
            self.output = output
        }

        func setOutput(_ output: Output) {
            lock.withLock { self.output = output }
        }

        func request(target: AlienTarget) async throws -> AlienResponse {
            try response(target: target)
        }

        private func response(target: AlienTarget) throws -> AlienResponse {
            try lock.withLock {
                if target.method == .get, target.body == nil {
                    switch target.url {
                    case Self.baseUrl + "/auth/paymail/config":
                        guard target.headers?["Authorization"]?.isEmpty == false else { throw rejected() }
                        managementRequests += 1
                        return try envelope([
                            "domains": ["sender.invalid"],
                            "publicBaseUrl": Self.origin,
                            "network": "BSV_MAINNET",
                            "addressType": "P2PKH",
                            "identityKeyRotation": false,
                        ])
                    case Self.baseUrl + "/auth/paymail/me":
                        guard target.headers?["Authorization"]?.isEmpty == false else { throw rejected() }
                        managementRequests += 1
                        return try envelope([["handle": "sender@sender.invalid", "address": address, "identityPublicKey": identityPublicKey, "active": true, "version": 1]])
                    case "https://example.invalid/.well-known/bsvalias":
                        guard onlyPublicHeaders(target) else { throw rejected() }
                        publicRequests += 1
                        return try response(["bsvalias": "1.0", "capabilities": ["paymentDestination": "https://example.invalid/pay/{alias}@{domain.tld}"]])
                    default: break
                    }
                }
                if target.method == .post, target.url == "https://example.invalid/pay/receiver@example.invalid", onlyPublicHeaders(target),
                   let data = target.body,
                   let body = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   body["senderHandle"] as? String == "sender@sender.invalid",
                   body["dt"] is String,
                   Set(body.keys).isSubset(of: ["senderHandle", "dt", "amount"])
                {
                    publicRequests += 1
                    let script: String = switch output {
                    case .p2pkh: "76a914000000000000000000000000000000000000000088ac"
                    case .p2ms: lockingScript
                    }
                    return try response(["output": script])
                }
                throw rejected()
            }
        }

        private func onlyPublicHeaders(_ target: AlienTarget) -> Bool {
            Set(target.headers?.keys.map { $0.lowercased() } ?? []).isSubset(of: ["content-type", "cache-control"])
        }

        private func envelope(_ data: Any) throws -> AlienResponse {
            try response(["success": true, "forceLogout": false, "code": "OK", "data": data])
        }

        private func response(_ data: Any) throws -> AlienResponse {
            try AlienResponse(status: 200, data: JSONSerialization.data(withJSONObject: data))
        }

        private func rejected() -> AlienError {
            rejectedRequests += 1
            return .RequestError(msg: "Public Paymail fixture forbids this request")
        }

        func rejectCredentials() -> BsvHomeServiceError {
            lock.withLock { credentials += 1 }
            return .Unsupported(msg: "Public Paymail fixture forbids credentials and signing")
        }

        func rejectStorage() -> BsvHomeServiceError {
            lock.withLock { rejectedRequests += 1 }
            return .Unsupported(msg: "Public Paymail fixture storage is read-only")
        }
    }

    private final class Password: BsvHomeKeystorePassword {
        let provider: Provider

        init(provider: Provider) {
            self.provider = provider
        }

        func getPassword(createIfMissing _: Bool) throws -> String {
            throw provider.rejectCredentials()
        }

        func getWalletPassword(walletId _: String) throws -> String? {
            throw provider.rejectCredentials()
        }

        func deleteWalletPassword(walletId _: String) throws {
            throw provider.rejectCredentials()
        }

        func authentication() throws -> BsvHomeKeystoreAuthentication {
            throw provider.rejectCredentials()
        }
    }

    private final class Wallets: BsvHomeWalletStore {
        let wallet: BsvHomeCore.Wallet
        let provider: Provider

        init(wallet: BsvHomeCore.Wallet, provider: Provider) {
            self.wallet = wallet; self.provider = provider
        }

        func getWallets() async throws -> [BsvHomeCore.Wallet] {
            [wallet]
        }

        func getWallet(walletId: String) async throws -> BsvHomeCore.Wallet? {
            walletId == wallet.id ? wallet : nil
        }

        func addWallet(wallet _: BsvHomeCore.Wallet) async throws {
            throw provider.rejectStorage()
        }

        func addWalletWithBsvAddresses(wallet _: BsvHomeCore.Wallet, addresses _: BsvHomeBsvAddressStoredState?) async throws {
            throw provider.rejectStorage()
        }

        func deleteWallet(walletId _: String) async throws -> Bool {
            throw provider.rejectStorage()
        }

        func setPinned(walletId _: String, pinned _: Bool) async throws {
            throw provider.rejectStorage()
        }

        func setName(walletId _: String, name _: String) async throws {
            throw provider.rejectStorage()
        }

        func setImageUrl(walletId _: String, imageUrl _: String?) async throws {
            throw provider.rejectStorage()
        }
    }

    private final class Session: BsvHomeWalletSessionStore {
        let walletId: String
        let provider: Provider

        init(walletId: String, provider: Provider) {
            self.walletId = walletId; self.provider = provider
        }

        func getCurrentWalletId() throws -> String? {
            walletId
        }

        func setCurrentWalletId(walletId: String?) throws {
            guard walletId == self.walletId else { throw provider.rejectStorage() }
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

    private final class CredentialStore: BsvHomeSecureStore {
        let provider: Provider

        init(provider: Provider) {
            self.provider = provider
        }

        func get(key _: String) throws -> String? {
            throw provider.rejectCredentials()
        }

        func set(key _: String, value _: String) throws {
            throw provider.rejectCredentials()
        }

        func remove(key _: String) throws {
            throw provider.rejectCredentials()
        }
    }

    private final class StakeStore: BsvHomeStakeStore {
        let provider: Provider

        init(provider: Provider) {
            self.provider = provider
        }

        func getApr(assetId _: BsvHomeCore.AssetId, providerType _: StakeProviderType) async throws -> Double? {
            throw provider.rejectStorage()
        }

        func getValidators(assetId _: BsvHomeCore.AssetId, providerType _: StakeProviderType) async throws -> [DelegationValidator] {
            throw provider.rejectStorage()
        }

        func saveValidators(validators _: [DelegationValidator]) async throws {
            throw provider.rejectStorage()
        }

        func deactivateValidators(assetId _: BsvHomeCore.AssetId, validatorIds _: [String]) async throws {
            throw provider.rejectStorage()
        }

        func getDelegationIds(walletId _: String, assetId _: BsvHomeCore.AssetId, providerType _: StakeProviderType) async throws -> [String] {
            throw provider.rejectStorage()
        }

        func updateDelegations(walletId _: String, delegations _: [DelegationBase], deleteIds _: [String]) async throws {
            throw provider.rejectStorage()
        }
    }
}
