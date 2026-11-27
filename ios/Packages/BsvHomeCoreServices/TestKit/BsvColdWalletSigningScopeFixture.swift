import Foundation
import BsvHomeCore
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import Primitives
import PrimitivesTestKit
import Store
import StoreTestKit

public final class BsvColdWalletSigningScopeFixture: @unchecked Sendable {
    public typealias Signing = @Sendable (BsvHomeCore.Wallet, String, String, BsvHomeBsvColdWalletActionScope) async throws -> String

    public let service: BsvHomeBsvColdWalletService
    public let wallet: Primitives.Wallet
    public let pending: BsvHomeBsvColdWalletPending
    public let password: MockKeystorePassword
    private let provider: Provider

    public var networkRequestCount: Int { provider.requestCount }

    private init(service: BsvHomeBsvColdWalletService, wallet: Primitives.Wallet, pending: BsvHomeBsvColdWalletPending, password: MockKeystorePassword, provider: Provider) {
        self.service = service
        self.wallet = wallet
        self.pending = pending
        self.password = password
        self.provider = provider
    }

    public static func make(directory: URL, coldStore: any BsvHomeBsvColdWalletStore, onSigning: @escaping Signing = { _, _, _, _ in throw BsvHomeServiceError.Cancelled }) async throws -> BsvColdWalletSigningScopeFixture {
        let vector = try JSONDecoder().decode(Vector.self, from: signingVectorData())
        guard let stateURL = Bundle.module.url(forResource: "bsv-cold-native-action-scope", withExtension: "json") else {
            throw BsvHomeServiceError.Unsupported(msg: "Verified public cold address state fixture is unavailable")
        }
        let resource = try JSONDecoder().decode(PublicState.self, from: Data(contentsOf: stateURL))
        guard resource.formatVersion == 1, resource.addressRevision == 0 else {
            throw BsvHomeServiceError.Unsupported(msg: "Verified public cold address state fixture is unsupported")
        }
        let wallet = resource.wallet
        let password = MockKeystorePassword()
        let keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("empty-keys", isDirectory: true), keystorePassword: password, environment: .wowPay, coldWalletStore: coldStore)
        let db = DB.mock()
        try AssetStore(db: db).add(assets: [.mock(asset: .mock(id: Primitives.Chain.bitcoinSv.assetId, name: "Bitcoin SV", symbol: "BSV"))])
        let nativeWallets = WalletStore(db: db)
        try nativeWallets.addWallet(wallet, bsvAddresses: (revision: resource.addressRevision, value: resource.addressValue))
        let wallets = BsvHomeWalletService.mock(keystore: keystore, db: db, sessionStore: .mock(), environment: .wowPay)
        try wallets.setCurrentWalletId(walletId: wallet.id.id)
        let provider = Provider()
        let gateway = BsvHomeGateway.newWithEnvironment(provider: provider, preferences: BsvHomePreferencesStoreMock(), securePreferences: BsvHomeSecureStoreMock(), environment: .wowPay)
        let addresses = BsvHomeBsvAddressPool(gateway: gateway, walletStore: BsvHomeCoreWalletStore(store: nativeWallets), store: BsvHomeCoreBsvAddressStore(store: BsvAddressPoolStore(db: db)), keystore: keystore.bsvHomeKeystore, password: BsvHomeCoreKeystorePassword(keystore: keystore))
        let service = BsvHomeBsvColdWalletService.offline(wallet: wallets, addresses: addresses, keystore: keystore.bsvHomeKeystore, store: coldStore, signer: Signer(signing: onSigning))
        let pending = try await service.importRequest(walletId: wallet.id.id, requestJson: vector.requestJson)
        guard pending.requestJson == vector.requestJson, pending.reviewJson == vector.reviewJson, pending.signedJson == nil else {
            throw BsvHomeServiceError.Unsupported(msg: "Public cold signing fixture review did not match the actual SDK")
        }
        return BsvColdWalletSigningScopeFixture(service: service, wallet: wallet, pending: pending, password: password, provider: provider)
    }

    public static func signingVectorData() throws -> Data {
        guard let url = Bundle.module.url(forResource: "bsv-cold-signing-bridge", withExtension: "json") else {
            throw BsvHomeServiceError.Unsupported(msg: "Public cold signing fixture is unavailable")
        }
        return try Data(contentsOf: url)
    }

    public func beginAction(intent: BsvHomeBsvColdWalletActionIntent) throws -> BsvHomeBsvColdWalletActionScope {
        try service.beginAction(walletId: wallet.id.id, intent: intent)
    }

    public func beginSigning() throws -> BsvHomeBsvColdWalletActionScope {
        try beginAction(intent: .sign(commitment: pending.review.commitment))
    }

    public func sign(scope: BsvHomeBsvColdWalletActionScope) async throws {
        _ = try await service.sign(walletId: wallet.id.id, commitment: pending.review.commitment, scope: scope)
    }

    private struct Vector: Decodable {
        let requestJson: String
        let reviewJson: String
    }

    private struct PublicState: Decodable {
        let formatVersion: UInt32
        let wallet: Primitives.Wallet
        let addressRevision: UInt64
        let addressValue: String
    }

    private final class Provider: AlienProvider, @unchecked Sendable {
        private let lock = NSLock()
        private var requests = 0
        var requestCount: Int { lock.withLock { requests } }

        func request(target _: AlienTarget) async throws -> AlienResponse {
            lock.withLock { requests += 1 }
            throw AlienError.RequestError(msg: "Public cold scope fixture forbids network access")
        }
    }

    private final class Signer: BsvHomeBsvColdWalletSigner, @unchecked Sendable {
        private let signing: Signing
        private let secureStore = BsvHomeSecureStoreMock()

        init(signing: @escaping Signing) {
            self.signing = signing
        }

        func authenticateImport() async throws {
            throw BsvHomeServiceError.Unsupported(msg: "Public cold scope fixture forbids secret import")
        }

        func authorize(wallet _: BsvHomeCore.Wallet, scope: BsvHomeBsvColdWalletActionScope) async throws {
            let password = try BsvHomeOfflineSigningPasswordService.forScope(store: secureStore, scope: scope)
            let status = try await password.status()
            if status.configured {
                try await password.verify(snapshot: status, password: "public-fixture-password")
            } else {
                _ = try await password.set(snapshot: status, currentPassword: nil, newPassword: "public-fixture-password", confirmation: "public-fixture-password")
            }
        }

        func authenticate(wallet _: BsvHomeCore.Wallet, scope: BsvHomeBsvColdWalletActionScope) async throws {
            try await scope.validate()
        }

        func signRequest(wallet: BsvHomeCore.Wallet, requestJson: String, reviewJson: String, scope: BsvHomeBsvColdWalletActionScope) async throws -> String {
            try await signing(wallet, requestJson, reviewJson, scope)
        }
    }
}
