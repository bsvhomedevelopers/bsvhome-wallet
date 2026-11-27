import Foundation
import BsvHomeCore
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit

final class BsvColdWalletServiceMock: BsvHomeBsvColdWalletService, @unchecked Sendable {
    var createReply: @Sendable () async throws -> BsvHomeBsvColdWalletSnapshot = { BsvColdWalletServiceMock.fixture }
    var exportPairingReply: @Sendable (String, UInt64) async throws -> BsvHomeBsvColdWalletSnapshot = { _, _ in BsvColdWalletServiceMock.fixture }
    var snapshotReply: @Sendable () async throws -> BsvHomeBsvColdWalletSnapshot = { BsvColdWalletServiceMock.fixture }
    var importReply: @Sendable () async throws -> BsvHomeBsvColdWalletSnapshot = { BsvColdWalletServiceMock.fixture }
    var broadcastReply: @Sendable () async throws -> BsvHomeBsvColdWalletPending = { BsvColdWalletServiceMock.signed }
    var importRequestReply: @Sendable () async throws -> BsvHomeBsvColdWalletPending = { BsvColdWalletServiceMock.signed }
    var selectWalletReply: @Sendable (String) async throws -> Void = { _ in }
    var importSignedReply: @Sendable () async throws -> BsvHomeBsvColdWalletPending = { BsvColdWalletServiceMock.signed }
    var refreshReply: @Sendable () async throws -> BsvHomeBsvColdWalletSnapshot = { BsvColdWalletServiceMock.fixture }
    var assembleReply: @Sendable ([String]) throws -> String? = { _ in nil }
    let calls = BsvColdWalletMockCalls()
    private let lock = NSLock()
    private var generation: UInt64 = 1
    private(set) var invalidations = 0
    private(set) var lastScope: BsvHomeBsvColdWalletActionScope?
    private var scopeFixture: BsvColdWalletSigningScopeFixture?
    private var fixtureDirectory: URL?
    var candidatesReply: @Sendable () async throws -> [BsvHomeBsvColdWalletCandidate] = { [.init(wallet: BsvColdWalletServiceMock.fixture.wallet, role: BsvColdWalletServiceMock.fixture.role)] }
    var selectAddressReply: @Sendable (String, UInt32) async throws -> BsvHomeBsvColdAddressState = { _, _ in BsvColdWalletServiceMock.fixture.addresses }
    var selectionReply: @Sendable (String) throws -> Bool = { _ in false }
    var currentWalletIdReply: @Sendable () throws -> String? = { nil }
    var prepareReply: @Sendable () async throws -> BsvHomeBsvColdWalletPending = { BsvColdWalletServiceMock.signed }
    var signReply: @Sendable () async throws -> BsvHomeBsvColdWalletPending = { BsvColdWalletServiceMock.signed }
    var signWithScopeReply: (@Sendable (BsvHomeBsvColdWalletActionScope) async throws -> BsvHomeBsvColdWalletPending)?
    var fixturePending: BsvHomeBsvColdWalletPending? { scopeFixture?.pending }

    @discardableResult
    func executeFixtureSign(scope: BsvHomeBsvColdWalletActionScope) async throws -> BsvHomeBsvColdWalletPending {
        guard let scopeFixture else { throw BsvHomeServiceError.Cancelled }
        return try await scopeFixture.service.sign(walletId: scopeFixture.wallet.id.id, commitment: scopeFixture.pending.review.commitment, scope: scope)
    }

    deinit {
        if let fixtureDirectory {
            try? FileManager.default.removeItem(at: fixtureDirectory)
        }
    }

    func installScopes(onSigning: @escaping BsvColdWalletSigningScopeFixture.Signing = { _, _, _, _ in throw BsvHomeServiceError.Cancelled }) async throws -> BsvHomeCore.Wallet {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        let fixture = try await BsvColdWalletSigningScopeFixture.make(directory: directory, coldStore: BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold")), onSigning: onSigning)
        scopeFixture = fixture
        fixtureDirectory = directory
        return fixture.wallet.map()
    }

    override func captureGeneration() -> UInt64 {
        lock.withLock { generation }
    }

    override func invalidateActions() throws {
        lock.withLock { generation += 1; invalidations += 1 }
    }

    override func consumeWalletSelection(walletId: String) throws -> Bool {
        try selectionReply(walletId)
    }

    override func beginAction(walletId: String, intent: BsvHomeBsvColdWalletActionIntent) throws -> BsvHomeBsvColdWalletActionScope {
        guard let scopeFixture, scopeFixture.wallet.id.id == walletId else { throw BsvHomeServiceError.Unsupported(msg: "Real scope fixture was not installed") }
        let scope = try scopeFixture.beginAction(intent: intent)
        lastScope = scope
        return scope
    }

    override func createWords(expectedGeneration _: UInt64) throws -> [String] {
        ["public", "fixture"]
    }

    override func qrFrames(payloadJson: String) throws -> [String] {
        [payloadJson]
    }

    override func walletCandidates() async throws -> [BsvHomeBsvColdWalletCandidate] {
        try await candidatesReply()
    }

    override func prepare(walletId _: String, recipient _: String, amount _: UInt64, scope _: BsvHomeBsvColdWalletActionScope) async throws -> BsvHomeBsvColdWalletPending {
        try await prepareReply()
    }

    override func sign(walletId _: String, commitment _: String, scope: BsvHomeBsvColdWalletActionScope) async throws -> BsvHomeBsvColdWalletPending {
        if let signWithScopeReply {
            return try await signWithScopeReply(scope)
        }
        return try await signReply()
    }

    override func create(name _: String, words _: [String], expectedGeneration _: UInt64) async throws -> BsvHomeBsvColdWalletSnapshot {
        await calls.created(); return try await createReply()
    }

    override func importRequest(walletId _: String, requestJson _: String) async throws -> BsvHomeBsvColdWalletPending {
        try await importRequestReply()
    }

    override func snapshot(walletId _: String) async throws -> BsvHomeBsvColdWalletSnapshot {
        try await snapshotReply()
    }

    override func exportPairing(walletId: String, expectedGeneration: UInt64) async throws -> BsvHomeBsvColdWalletSnapshot {
        await calls.exportedPairing(walletId: walletId, generation: expectedGeneration)
        return try await exportPairingReply(walletId, expectedGeneration)
    }

    override func secretKind(walletId _: String) async throws -> BsvHomeWalletSecretKind? {
        .recovery
    }

    override func wallets() async throws -> [BsvHomeCore.Wallet] {
        [Self.fixture.wallet]
    }

    override func selectWallet(walletId: String) async throws {
        try await selectWalletReply(walletId)
    }

    override func selectAddress(walletId: String, index: UInt32) async throws -> BsvHomeBsvColdAddressState {
        try await selectAddressReply(walletId, index)
    }

    override func currentWalletId() throws -> String? {
        try currentWalletIdReply()
    }

    override func qrAssemble(frames: [String]) throws -> String? {
        try assembleReply(frames)
    }

    override func previewRecovery(input _: String, accountPath _: String?, passphrase _: String?, expectedAddress _: String?, legacyNonCompliant _: Bool?, expectedGeneration _: UInt64) throws -> String {
        Self.pairing
    }

    override func previewObserver(input _: String, accountPath _: String?, expectedAddress _: String?) throws -> String {
        Self.pairing
    }

    override func importObserver(name _: String, pairingJson _: String, expectedGeneration _: UInt64) async throws -> BsvHomeBsvColdWalletSnapshot {
        try await importReply()
    }

    override func importRecovery(name _: String, input _: String, accountPath _: String?, passphrase _: String?, pairingJson _: String, legacyNonCompliant _: Bool?, expectedGeneration _: UInt64) async throws -> BsvHomeBsvColdWalletSnapshot {
        try await importReply()
    }

    override func broadcast(walletId _: String, commitment _: String, scope _: BsvHomeBsvColdWalletActionScope) async throws -> BsvHomeBsvColdWalletPending {
        try await broadcastReply()
    }

    override func importSigned(walletId _: String, signedJson _: String) async throws -> BsvHomeBsvColdWalletPending {
        try await importSignedReply()
    }

    override func refresh(walletId _: String) async throws -> BsvHomeBsvColdWalletSnapshot {
        try await refreshReply()
    }

    static var signed: BsvHomeBsvColdWalletPending {
        BsvHomeBsvColdWalletPending(requestJson: "fixture", reviewJson: "fixture", review: BsvHomeBsvColdWalletReview(commitment: "fixture-commitment", walletId: fixture.wallet.id, recipient: "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA", amount: 100, fee: 1, totalDebit: 101, change: 0, changeAddress: nil, inputCount: 1), signedJson: "fixture", transactionId: "fixture-transaction-id", broadcasted: false, persistenceError: nil)
    }

    static var pairing: String { "{\"accountPath\":\"m/44'/236'/0'\",\"firstAddress\":\"1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA\"}" }

    static var observerFixture: BsvHomeBsvColdWalletSnapshot {
        var value = fixture
        value.wallet.walletType = .view
        value.role = .observer
        return value
    }

    static var fixture: BsvHomeBsvColdWalletSnapshot {
        let wallet = BsvHomeCore.Wallet(id: "fixture-wallet", externalId: nil, name: "Fixture", index: 0, walletType: .single, accounts: [Account(chain: "bitcoinsv", address: "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA", derivationPath: "m/44'/236'/0'/0/0", extendedPublicKey: "fixture-xpub")], isPinned: false, imageUrl: nil, source: .create)
        return BsvHomeBsvColdWalletSnapshot(wallet: wallet, pairingJson: "fixture", addresses: .init(addresses: [], selectedIndex: 0, canAdd: false), balance: nil, transactions: [], pending: nil, role: .signer)
    }
}

extension BsvColdWalletServiceMock {
    final class SnapshotCursor: @unchecked Sendable {
        private let lock = NSLock()
        private let first: BsvHomeBsvColdWalletSnapshot
        private let second: BsvHomeBsvColdWalletSnapshot
        private var current: BsvHomeBsvColdWalletSnapshot

        init(first: BsvHomeBsvColdWalletSnapshot, second: BsvHomeBsvColdWalletSnapshot) {
            self.first = first
            self.second = second
            current = first
        }

        func snapshot() -> BsvHomeBsvColdWalletSnapshot {
            lock.withLock { current }
        }

        func select(_ id: String) throws {
            try lock.withLock {
                if id == first.wallet.id {
                    current = first
                } else if id == second.wallet.id {
                    current = second
                } else {
                    throw BsvHomeServiceError.Store(msg: "Unknown public snapshot selection")
                }
            }
        }
    }
}
