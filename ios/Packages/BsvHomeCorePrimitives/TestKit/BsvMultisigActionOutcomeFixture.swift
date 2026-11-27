import Foundation
import BsvHomeCore
import enum Primitives.Chain
import enum Primitives.WalletId

public final class BsvMultisigActionOutcomeFixture: Sendable {
    public enum Scenario: Sendable {
        case unknownAfterPost
        case submittedThenProjectionUnavailable
        case submittedThenStateCleanupUnavailable
    }

    public static let walletId = "native-action-outcome-wallet"
    public static let proposalId = "native-action-outcome-proposal"
    public let service: BsvHomeBsvMultisigService
    public let destination: String
    public let amount = "0.00042"
    public let memo = ""

    private let directory: URL
    private let observations: Observations
    private let state: StateStore

    public var lastScope: BsvHomeBsvMultisigActionScope? { observations.lastScope }
    public var businessPostCount: Int { observations.postCount }
    public var authorizationCount: Int { observations.authorizationCount }
    public var authenticationCount: Int { observations.authenticationCount }
    public var unexpectedRequestCount: Int { observations.unexpectedRequestCount }
    public var forbiddenCredentialCount: Int { observations.forbiddenCredentialCount }
    public var projectionReadFaultCount: Int { state.readFaultCount }
    public var successfulStateWriteCount: Int { state.writeCount }
    public var cleanupWriteFaultCount: Int { state.cleanupWriteFaultCount }

    private init(service: BsvHomeBsvMultisigService, destination: String, directory: URL, observations: Observations, state: StateStore) {
        self.service = service
        self.destination = destination
        self.directory = directory
        self.observations = observations
        self.state = state
    }

    deinit {
        try? FileManager.default.removeItem(at: directory)
    }

    public static func make(scenario: Scenario) async throws -> BsvMultisigActionOutcomeFixture {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        do {
            guard let url = Bundle.module.url(forResource: "bsv-multisig-public-proposal", withExtension: "json"),
                  let resource = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any],
                  let proposal = resource["proposal"] as? [String: Any],
                  let utxos = resource["utxos"] as? [[String: Any]],
                  let policy = proposal["policy"] as? [String: Any],
                  let threshold = policy["threshold"] as? UInt32,
                  let keys = policy["publicKeys"] as? [String],
                  let publicKey = keys.first,
                  let destination = proposal["recipient"] as? String,
                  (proposal["amount"] as? UInt64) == 42000
            else { throw failure("Public action fixture resource is unavailable") }
            let keystore = try BsvHomeKeystore.newWithEnvironment(baseDir: directory.path, environment: .wowPay)
            let address = try keystore.bsvMultisigMemberAddress(publicKey: publicKey)
            let localWalletId = Primitives.WalletId.privateKey(chain: .bitcoinSv, address: address).id
            let wallet = Wallet(id: localWalletId, externalId: nil, name: "Public action outcome fixture", index: 0, walletType: .privateKey,
                                accounts: [Account(chain: Primitives.Chain.bitcoinSv.rawValue, address: address, derivationPath: "", extendedPublicKey: nil)], isPinned: false, imageUrl: nil, source: .import)
            let wallets = Wallets(wallet: wallet)
            let session = BsvHomeWalletSessionService.newWithEnvironment(store: Session(walletId: localWalletId), wallets: wallets, environment: .wowPay)
            let descriptorJson = try keystore.bsvMultisigPolicy(threshold: threshold, publicKeys: keys)
            guard let descriptorData = descriptorJson.data(using: .utf8),
                  let descriptor = try JSONSerialization.jsonObject(with: descriptorData) as? [String: Any],
                  let configurationHash = descriptor["configurationHash"] as? String,
                  let lockingScript = descriptor["lockingScript"] as? String,
                  let scriptHash = descriptor["scriptHash"] as? String
            else { throw failure("Public action fixture policy is unavailable") }
            _ = try keystore.bsvMultisigReview(policyJson: String(decoding: JSONSerialization.data(withJSONObject: policy), as: UTF8.self),
                                               proposalJson: String(decoding: JSONSerialization.data(withJSONObject: proposal), as: UTF8.self))
            var members: [[String: Any]] = []
            for (index, key) in keys.enumerated() {
                try members.append(["userId": index + 1, "address": keystore.bsvMultisigMemberAddress(publicKey: key), "publicKey": key, "confirmed": true])
            }
            let origin = "https://action-outcome.invalid"
            let baseUrl = origin + "/api/portal"
            let remote: [String: Any] = ["id": walletId, "name": "Public Core action fixture", "status": "ACTIVE", "threshold": threshold,
                                         "memberCount": keys.count, "version": 7, "members": members, "configurationHash": configurationHash,
                                         "lockingScript": lockingScript, "scriptHash": scriptHash]
            let timestamp = Int64(Date().timeIntervalSince1970)
            let prepared: [String: Any] = ["id": proposalId, "walletId": walletId, "status": "COLLECTING", "creatorId": 1, "creatorAddress": address,
                                           "version": 3, "createdAt": timestamp, "expiresAt": timestamp + 86400, "note": "", "confirmations": 0,
                                           "proposal": proposal, "signatures": []]
            let initial: [String: Any] = ["trusted": ["\(origin)|\(localWalletId)|\(walletId)": policy], "memberNames": [:], "intentions": [:], "funding": [:], "creations": [:]]
            let state = try StateStore(json: String(decoding: JSONSerialization.data(withJSONObject: initial), as: UTF8.self), scenario: scenario)
            let observations = Observations()
            let provider = try Provider(baseUrl: baseUrl, remote: JSONSerialization.data(withJSONObject: remote), proposal: JSONSerialization.data(withJSONObject: prepared), utxos: JSONSerialization.data(withJSONObject: utxos),
                                        destination: destination, scenario: scenario, observations: observations)
            let password = Password(observations: observations)
            let gateway = BsvHomeGateway(provider: provider, preferences: BsvHomePreferencesStoreMock(), securePreferences: SecureStore())
            let addresses = BsvHomeBsvAddressPool(gateway: gateway, walletStore: wallets, store: AddressStore(), keystore: keystore, password: password)
            let secure = SecureStore()
            try secure.set(key: "session", value: String(decoding: JSONSerialization.data(withJSONObject: [
                "base_url": baseUrl, "wallet_id": localWalletId, "address": address, "token": Provider.cachedFixtureToken, "expires_at": NSNull(),
            ]), as: UTF8.self))
            let auth = try BsvHomePortalAuthService.newForWallet(provider: provider, keystore: keystore, password: password, session: session,
                                                             deviceKey: BsvHomeDeviceKeyService(store: SecureStore()), store: secure, baseUrl: baseUrl, language: "en", deviceType: "4", wallet: wallet)
            let service = BsvHomeBsvMultisigService.newWithAuthorization(store: state, session: session, gateway: gateway,
                                                                     signer: Signer(wallet: wallet, publicKey: publicKey, observations: observations), auth: auth, addresses: addresses,
                                                                     walletId: localWalletId, authorization: Authorization(wallet: wallet, configurationHash: configurationHash, destination: destination, observations: observations), coldStore: BsvMultisigActionScopeFixture.ColdStore())
            _ = try await service.currentSigningIdentity()
            try await service.load()
            return BsvMultisigActionOutcomeFixture(service: service, destination: destination, directory: directory, observations: observations, state: state)
        } catch {
            try? FileManager.default.removeItem(at: directory)
            throw error
        }
    }

    public func cancel() {
        service.cancel()
    }

    private static func failure(_ message: String) -> BsvHomeServiceError {
        .Unsupported(msg: message)
    }

    private final class Observations: @unchecked Sendable {
        private let lock = NSLock()
        private var scope: BsvHomeBsvMultisigActionScope?
        private var posts = 0
        private var authorizations = 0
        private var authentications = 0
        private var unexpected = 0
        private var forbidden = 0

        var lastScope: BsvHomeBsvMultisigActionScope? { lock.withLock { scope } }
        var postCount: Int { lock.withLock { posts } }
        var authorizationCount: Int { lock.withLock { authorizations } }
        var authenticationCount: Int { lock.withLock { authentications } }
        var unexpectedRequestCount: Int { lock.withLock { unexpected } }
        var forbiddenCredentialCount: Int { lock.withLock { forbidden } }

        func submittedRequest() throws {
            try lock.withLock {
                posts += 1
                guard posts == 1 else { throw AlienError.RequestError(msg: "Fixture refuses a repeated business POST") }
            }
        }

        func authorized(_ scope: BsvHomeBsvMultisigActionScope) {
            lock.withLock { self.scope = scope; authorizations += 1 }
        }

        func authenticated() {
            lock.withLock { authentications += 1 }
        }

        func unexpectedRequest() -> AlienError {
            lock.withLock { unexpected += 1 }
            return .RequestError(msg: "Action outcome fixture rejects this route")
        }

        func forbiddenCredential() -> BsvHomeServiceError {
            lock.withLock { forbidden += 1 }
            return .Unsupported(msg: "Action outcome fixture forbids secrets and signing")
        }
    }

    private final class Provider: AlienProvider, @unchecked Sendable {
        static let cachedFixtureToken = "public-in-memory-action-fixture"
        let baseUrl: String
        let remote: Data
        let proposal: Data
        let utxos: Data
        let destination: String
        let scenario: Scenario
        let observations: Observations
        private let lock = NSLock()
        private var prepared = false

        init(baseUrl: String, remote: Data, proposal: Data, utxos: Data, destination: String, scenario: Scenario, observations: Observations) {
            self.baseUrl = baseUrl; self.remote = remote; self.proposal = proposal; self.utxos = utxos; self.destination = destination; self.scenario = scenario; self.observations = observations
        }

        func request(target: AlienTarget) async throws -> AlienResponse {
            guard target.headers?["Authorization"] == Self.cachedFixtureToken else { throw observations.unexpectedRequest() }
            let list = baseUrl + "/auth/multisig/wallets"
            let detail = list + "/" + BsvMultisigActionOutcomeFixture.walletId
            if target.method == .get {
                guard target.body == nil else { throw observations.unexpectedRequest() }
                switch target.url {
                case list: return try envelope([object(remote)])
                case detail: return try envelope(object(remote))
                case detail + "/utxos": return try envelope(object(utxos))
                case detail + "/proposal-page?offset=0&limit=200":
                    let items: [Any] = if lock.withLock({ prepared }) {
                        try [object(proposal)]
                    } else {
                        []
                    }
                    return try envelope(["items": items, "total": items.count, "offset": 0, "limit": 200, "hasMore": false])
                default: throw observations.unexpectedRequest()
                }
            }
            guard target.method == .post, target.url == detail + "/proposals/prepare", let body = target.body,
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any],
                  Set(json.keys) == Set(["expectedVersion", "idempotencyKey", "expiresInSeconds", "recipient", "amount", "note"]),
                  (json["expectedVersion"] as? UInt64) == 7,
                  (json["expiresInSeconds"] as? UInt64) == 86400,
                  json["recipient"] as? String == destination,
                  (json["amount"] as? UInt64) == 42000,
                  json["note"] as? String == "",
                  let key = json["idempotencyKey"] as? String, UUID(uuidString: key) != nil
            else { throw observations.unexpectedRequest() }
            try observations.submittedRequest()
            switch scenario {
            case .unknownAfterPost: throw AlienError.RequestError(msg: "Fixture response unavailable after the business POST")
            case .submittedThenProjectionUnavailable, .submittedThenStateCleanupUnavailable:
                lock.withLock { prepared = true }
                return try envelope(object(proposal))
            }
        }

        private func object(_ data: Data) throws -> Any {
            do { return try JSONSerialization.jsonObject(with: data) }
            catch { throw AlienError.RequestError(msg: "Public fixture JSON is invalid") }
        }

        private func envelope(_ data: Any) throws -> AlienResponse {
            do { return try AlienResponse(status: 200, data: JSONSerialization.data(withJSONObject: ["success": true, "code": "OK", "data": data])) }
            catch { throw AlienError.RequestError(msg: "Public fixture response is invalid") }
        }
    }

    private final class StateStore: BsvHomeBsvMultisigStore, @unchecked Sendable {
        private let lock = NSLock()
        private let scenario: Scenario
        private var json: String
        private var revision: UInt64 = 0
        private var hasIntention = false
        private var remainingProjectionReads: Int?
        private var faults = 0
        private var writes = 0
        private var cleanupFaults = 0

        var readFaultCount: Int { lock.withLock { faults } }
        var writeCount: Int { lock.withLock { writes } }
        var cleanupWriteFaultCount: Int { lock.withLock { cleanupFaults } }

        init(json: String, scenario: Scenario) {
            self.json = json; self.scenario = scenario
        }

        func getState() throws -> BsvHomeBsvMultisigStoredState? {
            try lock.withLock {
                if let remaining = remainingProjectionReads {
                    if remaining == 0 {
                        remainingProjectionReads = nil
                        faults += 1
                        throw BsvHomeServiceError.Unsupported(msg: "Fixture projection read is temporarily unavailable")
                    }
                    remainingProjectionReads = remaining - 1
                }
                return BsvHomeBsvMultisigStoredState(revision: revision, json: json)
            }
        }

        func compareAndSwap(expectedRevision: UInt64, json: String) async throws -> Bool {
            guard let data = json.data(using: .utf8), let state = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let intentions = state["intentions"] as? [String: Any]
            else { throw BsvHomeServiceError.Unsupported(msg: "Fixture state JSON is invalid") }
            return try lock.withLock {
                guard expectedRevision == revision else { return false }
                if case .submittedThenStateCleanupUnavailable = scenario, hasIntention, intentions.isEmpty, cleanupFaults == 0 {
                    cleanupFaults += 1
                    throw BsvHomeServiceError.Unsupported(msg: "Fixture action cleanup is temporarily unavailable")
                }
                if case .submittedThenProjectionUnavailable = scenario, hasIntention, intentions.isEmpty, faults == 0 {
                    remainingProjectionReads = 1
                }
                hasIntention = !intentions.isEmpty
                revision += 1
                writes += 1
                self.json = json
                return true
            }
        }
    }

    private final class Authorization: BsvHomeBsvMultisigAuthorization {
        let wallet: Wallet
        let configurationHash: String
        let destination: String
        let observations: Observations

        init(wallet: Wallet, configurationHash: String, destination: String, observations: Observations) {
            self.wallet = wallet; self.configurationHash = configurationHash; self.destination = destination; self.observations = observations
        }

        func authorize(wallet: Wallet, request: BsvHomeBsvMultisigAuthorizationRequest, scope: BsvHomeBsvMultisigActionScope) async throws {
            guard wallet == self.wallet,
                  case let .prepareProposal(walletId, version, hash, destination, amount, memo) = request,
                  walletId == BsvMultisigActionOutcomeFixture.walletId, version == 7, hash == configurationHash,
                  destination == self.destination, amount == "42000", memo == ""
            else { throw observations.forbiddenCredential() }
            do { try scope.check() } catch { throw BsvHomeServiceError.Cancelled }
            observations.authorized(scope)
        }
    }

    private final class Signer: BsvHomeBsvMultisigSigner {
        let wallet: Wallet
        let publicKey: String
        let observations: Observations

        init(wallet: Wallet, publicKey: String, observations: Observations) {
            self.wallet = wallet; self.publicKey = publicKey; self.observations = observations
        }

        func publicKey(wallet: Wallet) async throws -> String {
            guard wallet == self.wallet else { throw observations.forbiddenCredential() }
            return publicKey
        }

        func authenticate(wallet: Wallet, scope: BsvHomeBsvMultisigActionScope?) async throws {
            guard wallet == self.wallet, let scope else { throw observations.forbiddenCredential() }
            do { try scope.check() } catch { throw BsvHomeServiceError.Cancelled }
            observations.authenticated()
        }

        func signProposal(wallet _: Wallet, policyJson _: String, proposalJson _: String, recipient _: String, amount _: UInt64, maximumFee _: UInt64, scope _: BsvHomeBsvMultisigActionScope) async throws -> String {
            throw observations.forbiddenCredential()
        }

        func signFunding(wallet _: Wallet, fundingJson _: String, reviewJson _: String, scope _: BsvHomeBsvMultisigActionScope) async throws -> String {
            throw observations.forbiddenCredential()
        }
    }

    private final class Password: BsvHomeKeystorePassword {
        let observations: Observations
        init(observations: Observations) {
            self.observations = observations
        }

        func getPassword(createIfMissing _: Bool) throws -> String {
            throw observations.forbiddenCredential()
        }

        func getWalletPassword(walletId _: String) throws -> String? {
            throw observations.forbiddenCredential()
        }

        func deleteWalletPassword(walletId _: String) throws {
            throw observations.forbiddenCredential()
        }

        func authentication() throws -> BsvHomeKeystoreAuthentication {
            throw observations.forbiddenCredential()
        }
    }

    private final class Wallets: BsvHomeWalletStore {
        let wallet: Wallet
        init(wallet: Wallet) {
            self.wallet = wallet
        }

        func getWallets() async throws -> [Wallet] {
            [wallet]
        }

        func getWallet(walletId: String) async throws -> Wallet? {
            walletId == wallet.id ? wallet : nil
        }

        func addWallet(wallet _: Wallet) async throws {
            throw BsvMultisigActionOutcomeFixture.failure("Fixture wallet is read-only")
        }

        func addWalletWithBsvAddresses(wallet _: Wallet, addresses _: BsvHomeBsvAddressStoredState?) async throws {
            throw BsvMultisigActionOutcomeFixture.failure("Fixture wallet is read-only")
        }

        func deleteWallet(walletId _: String) async throws -> Bool {
            throw BsvMultisigActionOutcomeFixture.failure("Fixture wallet is read-only")
        }

        func setPinned(walletId _: String, pinned _: Bool) async throws {
            throw BsvMultisigActionOutcomeFixture.failure("Fixture wallet is read-only")
        }

        func setName(walletId _: String, name _: String) async throws {
            throw BsvMultisigActionOutcomeFixture.failure("Fixture wallet is read-only")
        }

        func setImageUrl(walletId _: String, imageUrl _: String?) async throws {
            throw BsvMultisigActionOutcomeFixture.failure("Fixture wallet is read-only")
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
            guard walletId == self.walletId else { throw BsvMultisigActionOutcomeFixture.failure("Fixture current wallet is fixed") }
        }
    }

    private final class AddressStore: BsvHomeBsvAddressStore {
        func fundingStates() async throws -> [BsvHomeBsvFundingStoredState] {
            []
        }

        func get(walletId _: String) async throws -> BsvHomeBsvAddressStoredState? {
            nil
        }

        func save(walletId _: String, expectedRevision _: UInt64?, state _: BsvHomeBsvAddressStoredState) async throws -> Bool {
            throw BsvMultisigActionOutcomeFixture.failure("Fixture forbids address allocation")
        }

        func unresolvedTransactions(walletId _: String) async throws -> [Transaction] {
            []
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
