import BsvMultisig
import Foundation
@testable import BsvHome
import BsvHomeCore
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import Localization
import Primitives
import Testing

@MainActor
struct CollectiveWalletDirectoryTests {
    @Test
    func reusesModelsAndCancelsRemovedSignersWithoutChangingTheSession() async throws {
        let signer = wallet("First")
        let observer = wallet("Observer", address: secondAddress, type: .view)
        let session = Session(noHandle: .init())
        session.replace([signer, observer], currentWalletId: signer.id.id)
        let service = BsvMultisigServiceMock()
        var made: [Primitives.Wallet] = []
        let directory = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: BsvHomePreferencesService(store: BsvHomePreferencesStoreMock()), origin: "https://paymail.bsvhome.com/api/portal") { wallet in
            made.append(wallet)
            return BsvMultisigViewModel(makeService: { service })
        }
        #expect(made.isEmpty)
        #expect(service.calls.isEmpty)

        await directory.refresh()
        let first = try #require(directory.model(for: signer.id))
        await directory.refresh()

        #expect(directory.model(for: signer.id) === first)
        #expect(directory.model(for: observer.id) == nil)
        #expect(made == [signer])
        #expect(service.calls.filter { $0 == "load" }.count == 2)
        #expect(session.projectedWalletReads == 0)
        #expect(session.selectionChanges == 0)

        session.replace([observer], currentWalletId: signer.id.id)
        await directory.refresh()

        #expect(directory.model(for: signer.id) == nil)
        #expect(directory.wallets.isEmpty)
        #expect(service.calls.contains("cancel"))
        #expect(directory.errorMessage == nil)
    }

    @Test
    func rebuildingFixedIdentityKeepsTheCompleteNewWallet() async throws {
        let first = wallet("First")
        let replacement = wallet("Replacement", address: secondAddress, id: first.id)
        let session = Session(noHandle: .init())
        session.replace([first], currentWalletId: first.id.id)
        let oldService = BsvMultisigServiceMock()
        let newService = BsvMultisigServiceMock()
        var made: [Primitives.Wallet] = []
        let directory = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: BsvHomePreferencesService(store: BsvHomePreferencesStoreMock()), origin: "https://paymail.bsvhome.com/api/portal") { wallet in
            made.append(wallet)
            let service = made.count == 1 ? oldService : newService
            return BsvMultisigViewModel(makeService: { service })
        }
        await directory.refresh()
        let previous = try #require(directory.model(for: first.id))

        session.replace([replacement], currentWalletId: first.id.id)
        await directory.refresh()

        #expect(directory.model(for: first.id) !== previous)
        #expect(made == [first, replacement])
        #expect(oldService.calls.contains("cancel"))
        #expect(newService.calls == ["load"])
        #expect(session.selectionChanges == 0)
    }

    @Test(.timeLimit(.minutes(1)))
    func refreshesSignersIndependentlyAndShowsEachFailure() async {
        let first = wallet("First")
        let second = wallet("Second", address: secondAddress)
        let session = Session(noHandle: .init())
        session.replace([first, second], currentWalletId: second.id.id)
        let firstGate = BsvMultisigAsyncGate()
        let secondGate = BsvMultisigAsyncGate()
        let firstService = BsvMultisigServiceMock(loadAction: {
            await firstGate.wait()
            throw BsvHomeBsvMultisigError.Provider(reason: "fixture")
        })
        let secondService = BsvMultisigServiceMock(loadAction: {
            await secondGate.wait()
            throw BsvHomeBsvMultisigError.Storage(reason: "fixture")
        })
        let directory = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: BsvHomePreferencesService(store: BsvHomePreferencesStoreMock()), origin: "https://paymail.bsvhome.com/api/portal") { wallet in
            let service = wallet.id == first.id ? firstService : secondService
            return BsvMultisigViewModel(makeService: { service })
        }
        let refreshing = Task { await directory.refresh() }
        await firstGate.started()
        await secondGate.started()
        #expect(directory.isLoading)

        await firstGate.release()
        await secondGate.release()
        await refreshing.value

        #expect(!directory.isLoading)
        #expect(directory.errorMessage?.contains("First") == true)
        #expect(directory.errorMessage?.contains("Second") == true)
        #expect(directory.errorMessage?.contains(Localized.multisigErrorProvider) == true)
        #expect(directory.errorMessage?.contains(Localized.multisigErrorStorage) == true)
        #expect(session.selectionChanges == 0)
    }

    @Test(.timeLimit(.minutes(1)))
    func cancellingASelectorWaiterKeepsTheSharedRefreshAndSnapshot() async throws {
        let signer = wallet("First")
        let session = Session(noHandle: .init())
        session.replace([signer], currentWalletId: signer.id.id)
        let gate = BsvMultisigAsyncGate()
        let service = BsvMultisigServiceMock(loadAction: { await gate.wait() })
        let directory = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: BsvHomePreferencesService(store: BsvHomePreferencesStoreMock()), origin: "https://paymail.bsvhome.com/api/portal") { _ in
            BsvMultisigViewModel(makeService: { service })
        }
        let selectorWaiter = Task { await directory.refresh() }
        await gate.started()
        let model = try #require(directory.model(for: signer.id))

        selectorWaiter.cancel()
        await gate.release()
        await selectorWaiter.value

        #expect(directory.model(for: signer.id) === model)
        #expect(!directory.wallets.isEmpty)
        #expect(!service.calls.contains("cancel"))
        #expect(service.calls.filter { $0 == "load" }.count == 1)
        #expect(!directory.isLoading)
        #expect(directory.errorMessage == nil)
        #expect(session.selectionChanges == 0)
    }

    @Test(.timeLimit(.minutes(1)))
    func cancelledResponsesCannotReplaceARebuiltDirectory() async throws {
        let signer = wallet("First")
        let session = Session(noHandle: .init())
        session.replace([signer], currentWalletId: signer.id.id)
        let gate = BsvMultisigAsyncGate()
        let cancelledService = BsvMultisigServiceMock(loadAction: { await gate.wait() })
        let replacementService = BsvMultisigServiceMock(recordsAreEmpty: true)
        var made = 0
        let directory = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: BsvHomePreferencesService(store: BsvHomePreferencesStoreMock()), origin: "https://paymail.bsvhome.com/api/portal") { _ in
            made += 1
            let service = made == 1 ? cancelledService : replacementService
            return BsvMultisigViewModel(makeService: { service })
        }
        let previousRefresh = Task { await directory.refresh() }
        await gate.started()
        let cancelledModel = directory.model(for: signer.id)
        directory.cancel()
        #expect(directory.model(for: signer.id) == nil)
        #expect(directory.wallets.isEmpty)

        await directory.refresh()
        let replacementModel = directory.model(for: signer.id)
        await gate.release()
        await previousRefresh.value

        let replacement = try #require(replacementModel)
        let previous = try #require(cancelledModel)
        #expect(replacement !== previous)
        #expect(directory.model(for: signer.id) === replacement)
        #expect(directory.wallets.isEmpty)
        #expect(cancelledService.calls.contains("cancel"))
        #expect(!directory.isLoading)
        #expect(directory.errorMessage == nil)
        #expect(made == 2)
    }

    @Test
    func sourceFailureCancelsCachedModelsAndRemainsVisible() async {
        let signer = wallet("First")
        let session = Session(noHandle: .init())
        session.replace([signer], currentWalletId: signer.id.id)
        let service = BsvMultisigServiceMock()
        let directory = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: BsvHomePreferencesService(store: BsvHomePreferencesStoreMock()), origin: "https://paymail.bsvhome.com/api/portal") { _ in
            BsvMultisigViewModel(makeService: { service })
        }
        await directory.refresh()
        session.fail()

        await directory.refresh()

        #expect(directory.model(for: signer.id) == nil)
        #expect(directory.wallets.isEmpty)
        #expect(directory.errorMessage == Localized.multisigErrorStorage)
        #expect(service.calls.contains("cancel"))
        #expect(!directory.isLoading)
    }

    @Test
    func malformedRawSignerIsReportedInsteadOfBeingFilteredByTheSession() async {
        let malformed = wallet("Malformed", address: "invalid-bsv-address")
        let session = Session(noHandle: .init())
        session.replace([malformed], currentWalletId: malformed.id.id)
        var made = 0
        let directory = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: BsvHomePreferencesService(store: BsvHomePreferencesStoreMock()), origin: "https://paymail.bsvhome.com/api/portal") { _ in
            made += 1
            return BsvMultisigViewModel(makeService: { BsvMultisigServiceMock() })
        }

        await directory.refresh()

        #expect(made == 0)
        #expect(session.projectedWalletReads == 0)
        #expect(directory.errorMessage == Localized.multisigErrorMissingSigner)
        #expect(directory.wallets.isEmpty)
    }

    @Test
    func remembersIndependentSignerSelectionsAcrossRestartWithoutChangingPersonalCurrent() async throws {
        let first = wallet("First")
        let second = wallet("Second", address: secondAddress)
        let session = Session(noHandle: .init())
        session.replace([first, second], currentWalletId: first.id.id)
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let origin = "https://paymail.bsvhome.com/api/portal"
        try preferences.setMultisigSigner(origin: origin, walletId: "a", signerWalletId: second.id.id)
        try preferences.setMultisigSigner(origin: origin, walletId: "b", signerWalletId: first.id.id)
        let makeModel: @MainActor (Primitives.Wallet) -> BsvMultisigViewModel = { signer in
            let service = BsvMultisigServiceMock()
            service.walletRecords = ["a", "b"].map { id in
                var row = BsvMultisigFixtures.wallet(id: id)
                row.localSignerWalletId = signer.id.id
                return row
            }
            return BsvMultisigViewModel(makeService: { service })
        }
        let directory = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: preferences, origin: origin, makeModel: makeModel)
        await directory.refresh()
        #expect(directory.wallets.map(\.localSignerWalletId) == [second.id.id, first.id.id])
        #expect(directory.identities(for: "a").map(\.id) == [first.id, second.id])
        let previous = try #require(directory.model(for: second.id))
        try directory.selectSigner(first.id, for: "a")
        await directory.refresh()
        #expect(directory.model(for: second.id) !== previous)
        #expect(directory.wallets.map(\.localSignerWalletId) == [first.id.id, first.id.id])

        directory.cancel()
        let restored = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: preferences, origin: origin, makeModel: makeModel)
        await restored.refresh()
        #expect(restored.wallets.map(\.localSignerWalletId) == [first.id.id, first.id.id])
        #expect(session.selectionChanges == 0)
    }

    @Test
    func globalPendingIncludesEveryIdentityAndPreservesTheChosenSigner() async throws {
        let first = wallet("First")
        let second = wallet("Second", address: secondAddress)
        let session = Session(noHandle: .init())
        session.replace([first, second], currentWalletId: first.id.id)
        let firstService = BsvMultisigServiceMock()
        let secondService = BsvMultisigServiceMock()
        let publicKeys = try BsvMultisigFixtures.paymentRequest().publicKeys
        let members = publicKeys.enumerated().map { index, publicKey in
            BsvHomeBsvMultisigMember(slot: UInt32(index + 1), name: "Member \(index + 1)", publicKey: publicKey, address: nil, status: .confirmed)
        }
        var firstWallet = BsvMultisigFixtures.wallet(id: "a")
        firstWallet.members = members
        firstWallet.localSignerWalletId = first.id.id
        firstWallet.localSignerPublicKey = publicKeys[0]
        firstWallet.version = 1
        firstWallet.pendingCount = 1
        var secondWallet = firstWallet
        secondWallet.localSignerWalletId = second.id.id
        secondWallet.localSignerPublicKey = publicKeys[1]
        secondWallet.version = 2
        secondWallet.pendingCount = 4
        var otherWallet = BsvMultisigFixtures.wallet(id: "b")
        otherWallet.members = members
        otherWallet.localSignerWalletId = first.id.id
        otherWallet.localSignerPublicKey = publicKeys[0]
        otherWallet.pendingCount = 2
        var firstProposal = BsvMultisigFixtures.proposal(canSign: true)
        firstProposal.walletId = "a"
        firstProposal.expiresAt = UInt64(Date().addingTimeInterval(86400).timeIntervalSince1970)
        var secondProposal = firstProposal
        secondProposal.canSign = false
        secondProposal.personalPending = false
        var otherProposal = firstProposal
        otherProposal.id = "other-proposal"
        otherProposal.walletId = "b"
        otherProposal.createdAt = 200
        firstService.walletRecords = [firstWallet, otherWallet]
        firstService.proposalRecords = [firstProposal, otherProposal]
        secondService.walletRecords = [secondWallet]
        secondService.proposalRecords = [secondProposal, otherProposal]
        let directory = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: BsvHomePreferencesService(store: BsvHomePreferencesStoreMock()), origin: "https://paymail.bsvhome.com/api/portal") { signer in
            let service = signer.id == first.id ? firstService : secondService
            return BsvMultisigViewModel(makeService: { service })
        }

        await directory.refresh()

        #expect(directory.wallets.map(\.localSignerWalletId) == [second.id.id, first.id.id])
        #expect(directory.overview.pendingProposals == [otherProposal, secondProposal])
        #expect(directory.overview.pendingCount == 2)
        firstWallet.version = secondWallet.version
        firstService.walletRecords = [firstWallet, otherWallet]
        try directory.selectSigner(first.id, for: "a")
        await directory.refresh()

        #expect(directory.overview.pendingProposals == [otherProposal, firstProposal])
        #expect(directory.overview.pendingCount == 2)
        #expect(session.selectionChanges == 0)
    }

    @Test
    func failedSignerPersistenceDoesNotSwitchOrCancelTheActiveIdentity() async throws {
        let first = wallet("First")
        let second = wallet("Second", address: secondAddress)
        let session = Session(noHandle: .init())
        session.replace([first, second], currentWalletId: first.id.id)
        let store = BsvHomePreferencesStoreMock()
        let preferences = BsvHomePreferencesService(store: store)
        let services = [first.id: BsvMultisigServiceMock(), second.id: BsvMultisigServiceMock()]
        for (id, service) in services {
            var row = BsvMultisigFixtures.wallet(id: "a")
            row.localSignerWalletId = id.id
            service.walletRecords = [row]
        }
        let directory = CollectiveWalletDirectory(session: session, loadWallets: session.loadWallets, preferences: preferences, origin: "https://paymail.bsvhome.com/api/portal") { signer in
            let service = services[signer.id]!
            return BsvMultisigViewModel(makeService: { service })
        }
        await directory.refresh()
        let failure = BsvHomeServiceError.Store(msg: "fixture preference failure")
        store.failWrites(with: failure)
        #expect(throws: failure) { try directory.selectSigner(second.id, for: "a") }
        #expect(directory.wallets.first?.localSignerWalletId == first.id.id)
        #expect(services[first.id]?.calls.contains("cancel") == false)
        #expect(session.selectionChanges == 0)
        store.failWrites(with: nil)
        try directory.selectSigner(second.id, for: "a")
        #expect(services[first.id]?.calls.contains("cancel") == true)
        #expect(directory.wallets.first?.localSignerWalletId == second.id.id)
    }

    private var secondAddress: String { "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH" }

    private func wallet(_ name: String, address: String = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5", type: Primitives.WalletType = .multicoin, id: Primitives.WalletId? = nil) -> Primitives.Wallet {
        Primitives.Wallet(
            id: id ?? .make(walletType: type, chain: .bitcoinSv, address: address),
            externalId: nil,
            name: name,
            index: 0,
            type: type,
            accounts: [
                Primitives.Account(chain: .bitcoinSv, address: address, derivationPath: "m/44'/236'/0'/0/0", extendedPublicKey: nil),
                Primitives.Account(chain: .ethereum, address: "public-retained-account", derivationPath: "m/44'/60'/0'/0/0", extendedPublicKey: nil),
            ],
            isPinned: false,
            imageUrl: nil,
            source: .create,
        )
    }
}

private final class Session: BsvHomeWalletSessionService, @unchecked Sendable {
    private enum Failure: Error {
        case unavailable
    }

    private let lock = NSLock()
    private var records: [BsvHomeCore.Wallet] = []
    private var currentWalletId: String?
    private var readFailure = false
    private var reads = 0
    private var changes = 0

    var projectedWalletReads: Int { lock.withLock { reads } }
    var selectionChanges: Int { lock.withLock { changes } }

    func replace(_ wallets: [Primitives.Wallet], currentWalletId: String?) {
        lock.withLock {
            records = wallets.map { $0.map() }
            self.currentWalletId = currentWalletId
            readFailure = false
        }
    }

    func fail() {
        lock.withLock { readFailure = true }
    }

    func loadWallets() async throws -> [BsvHomeCore.Wallet] {
        try lock.withLock {
            if readFailure {
                throw Failure.unavailable
            }
            return records
        }
    }

    override func getWallets() async throws -> [BsvHomeCore.Wallet] {
        lock.withLock {
            reads += 1
            return []
        }
    }

    override func getCurrentWalletId() throws -> String? {
        lock.withLock { currentWalletId }
    }

    override func setCurrentWalletId(walletId: String?) throws {
        lock.withLock {
            changes += 1
            currentWalletId = walletId
        }
    }
}
