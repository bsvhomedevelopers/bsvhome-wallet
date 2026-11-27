@testable import BsvColdWallet
import Components
import BsvHomeCore
import Testing

@MainActor
struct BsvColdWalletEntryGuardTests {
    @Test(arguments: [false, true])
    func disappearingBeforeTheFirstPollDoesNotEnterTheWalletService(broadcast: Bool) async throws {
        let entries = EntryCount()
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [.init(wallet: BsvColdWalletServiceMock.observerFixture.wallet, role: .observer)] }
        service.createReply = {
            await entries.entered()
            return BsvColdWalletServiceMock.observerFixture
        }
        service.broadcastReply = {
            await entries.entered()
            return BsvColdWalletServiceMock.signed
        }
        let model = BsvColdWalletViewModel(service: service, mode: broadcast ? .online : .offline)
        if broadcast {
            var snapshot = BsvColdWalletServiceMock.fixture
            snapshot.wallet = try await service.installScopes()
            snapshot.role = .observer
            model.snapshot = snapshot
            model.pending = BsvColdWalletServiceMock.signed
            try #require(model.canBroadcast)
            let navigation = NavigationPathState()
            navigation.append(BsvColdWalletRoute.broadcast)
            model.broadcast(navigation: navigation)
        } else {
            model.walletName = "Public fixture"
            model.words = ["public", "fixture"]
            model.isBackupConfirmed = true
            model.create()
        }
        let started = try #require(model.operationTask)
        model.disappear()
        await started.value
        #expect(await service.calls.creates == 0)
        #expect(await entries.count == 0)
        #expect(model.operationTask == nil)
        #expect(!model.isBusy)
        #expect(model.words.isEmpty)
        #expect(model.path.isEmpty)
        #expect(model.errorMessage == nil)
    }

    @Test
    func cancellingTheTaskBeforeItsFirstPollDoesNotEnterCreate() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [.init(wallet: BsvColdWalletServiceMock.observerFixture.wallet, role: .observer)] }
        let model = BsvColdWalletViewModel(service: service, mode: .offline)
        model.walletName = "Public fixture"
        model.words = ["public", "fixture"]
        model.isBackupConfirmed = true
        model.create()
        let started = try #require(model.operationTask)
        started.cancel()
        await started.value
        #expect(await service.calls.creates == 0)
        #expect(model.operationTask == nil)
        #expect(!model.isBusy)
        #expect(model.snapshot == nil)
        #expect(model.errorMessage == nil)
    }

    @Test
    func anInitialSnapshotFailureCanRetryThePreferredWallet() async {
        let snapshots = SnapshotReplies()
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [.init(wallet: BsvColdWalletServiceMock.observerFixture.wallet, role: .observer)] }
        service.snapshotReply = { try await snapshots.snapshot() }
        let model = BsvColdWalletViewModel(service: service, mode: .online, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: .receive)
        await model.load()
        #expect(await snapshots.count == 1)
        #expect(model.snapshot == nil)
        #expect(model.errorMessage != nil)
        #expect(model.operationTask == nil)
        await snapshots.allowSnapshots()
        await model.load()
        #expect(await snapshots.count == 2)
        #expect(model.wallet?.id == BsvColdWalletServiceMock.fixture.wallet.id)
        #expect(model.path == [.receive])
        #expect(model.errorMessage == nil)
        #expect(model.operationTask == nil)
        #expect(!model.isBusy)
    }

    @Test
    func anActionFailureWithACachedWalletDoesNotRestartInitialLoading() async throws {
        let snapshots = SnapshotReplies()
        await snapshots.allowSnapshots()
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [.init(wallet: BsvColdWalletServiceMock.observerFixture.wallet, role: .observer)] }
        service.snapshotReply = { try await snapshots.snapshot() }
        service.refreshReply = { throw BsvHomeServiceError.Store(msg: "Public fixture refresh failure") }
        let model = BsvColdWalletViewModel(service: service, mode: .online, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: .receive)
        await model.load()
        #expect(model.path == [.receive])
        model.path = []
        model.refresh()
        let refresh = try #require(model.operationTask)
        await refresh.value
        #expect(model.errorMessage != nil)
        await model.load()
        #expect(await snapshots.count == 1)
        #expect(model.wallet?.id == BsvColdWalletServiceMock.fixture.wallet.id)
        #expect(model.path.isEmpty)
        #expect(model.errorMessage != nil)
    }

    @Test
    func retryLoadOwnsTheSnapshotReadAndClearsItsFailureOnSuccess() async throws {
        let snapshots = SnapshotReplies()
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [.init(wallet: BsvColdWalletServiceMock.observerFixture.wallet, role: .observer)] }
        service.snapshotReply = { try await snapshots.snapshot() }
        let model = BsvColdWalletViewModel(service: service, mode: .online, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: .receive)
        await model.load()
        try #require(model.loadFailed)
        #expect(model.snapshot == nil)
        #expect(model.errorMessage != nil)
        await snapshots.allowSnapshots()
        model.retryLoad()
        let retry = try #require(model.operationTask)
        #expect(model.isBusy)
        await retry.value
        #expect(await snapshots.count == 2)
        #expect(model.wallet?.id == BsvColdWalletServiceMock.fixture.wallet.id)
        #expect(model.path == [.receive])
        #expect(!model.loadFailed)
        #expect(model.errorMessage == nil)
        #expect(model.operationTask == nil)
        #expect(!model.isBusy)
    }

    @Test
    func retryLoadDoesNotRestartLoadingForAnOrdinaryActionFailure() async throws {
        let snapshots = SnapshotReplies()
        await snapshots.allowSnapshots()
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [.init(wallet: BsvColdWalletServiceMock.observerFixture.wallet, role: .observer)] }
        service.snapshotReply = { try await snapshots.snapshot() }
        service.refreshReply = { throw BsvHomeServiceError.Store(msg: "Public fixture refresh failure") }
        let model = BsvColdWalletViewModel(service: service, mode: .online, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: .receive)
        await model.load()
        model.path = []
        model.refresh()
        let refresh = try #require(model.operationTask)
        await refresh.value
        let actionError = try #require(model.errorMessage)
        #expect(!model.loadFailed)
        model.retryLoad()
        let retry = model.operationTask
        #expect(retry == nil)
        await retry?.value
        #expect(await snapshots.count == 1)
        #expect(model.wallet?.id == BsvColdWalletServiceMock.fixture.wallet.id)
        #expect(model.path.isEmpty)
        #expect(model.errorMessage == actionError)
        #expect(!model.loadFailed)
        #expect(!model.isBusy)
    }

    @Test(arguments: [false, true])
    func aCancelledSnapshotDoesNotOfferFailureRetry(serviceCancellation: Bool) async {
        let reads = EntryCount()
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [.init(wallet: BsvColdWalletServiceMock.observerFixture.wallet, role: .observer)] }
        service.snapshotReply = {
            await reads.entered()
            if serviceCancellation {
                throw BsvHomeServiceError.Cancelled
            }
            throw CancellationError()
        }
        let model = BsvColdWalletViewModel(service: service, mode: .online, walletId: BsvColdWalletServiceMock.fixture.wallet.id)
        await model.load()
        #expect(await reads.count == 1)
        #expect(model.snapshot == nil)
        #expect(model.errorMessage == nil)
        #expect(!model.loadFailed)
        #expect(model.operationTask == nil)
        #expect(!model.isBusy)
        model.retryLoad()
        let retry = model.operationTask
        #expect(retry == nil)
        await retry?.value
        #expect(await reads.count == 1)
        #expect(!model.loadFailed)
    }

    private actor EntryCount {
        private(set) var count = 0

        func entered() {
            count += 1
        }
    }

    private actor SnapshotReplies {
        private(set) var count = 0
        private var failing = true

        func allowSnapshots() {
            failing = false
        }

        func snapshot() throws -> BsvHomeBsvColdWalletSnapshot {
            count += 1
            if failing {
                throw BsvHomeServiceError.Store(msg: "Public fixture snapshot failure")
            }
            return BsvColdWalletServiceMock.observerFixture
        }
    }
}
