@testable import BsvColdWallet
import Components
import BsvHomeCore
import Testing

@MainActor
struct BsvColdWalletStateTests {
    @Test
    func scanningAnAlreadyBroadcastSignedPayloadShowsTheKnownResultAndStorageWarning() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        var result = BsvColdWalletServiceMock.signed
        result.broadcasted = true
        result.persistenceError = "fixture storage failure"
        let imported = result
        service.importSignedReply = { imported }
        var snapshot = BsvColdWalletServiceMock.fixture
        snapshot.wallet.walletType = .view
        snapshot.role = .observer
        let model = BsvColdWalletViewModel(service: service, mode: .online)
        model.snapshot = snapshot
        model.startScan(.signed)
        try model.scan("{\"kind\":\"wowpay-bsv-signed\"}", scannerId: #require(model.scannerSession?.id))
        await model.operationTask?.value
        #expect(model.scanner == nil)
        #expect(model.path == [.result])
        #expect(model.pending?.broadcasted == true)
        #expect(model.pending?.persistenceError != nil)
        #expect(!model.canBroadcast)
    }

    @Test
    func offlineReloadRechecksFileProtectionAfterAStaleOperationMayHavePersisted() async {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        let model = BsvColdWalletViewModel(service: service, mode: .offline, protectStorage: { await service.calls.protected() })
        await model.load()
        #expect(await service.calls.protections == 1)
        model.sceneChanged(.background)
        model.isForeground = true
        await model.load()
        #expect(await service.calls.protections == 2)
    }

    @Test
    func broadcastSuccessWithAStorageWarningKeepsTheResultAndDisablesRebroadcast() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        var result = BsvColdWalletServiceMock.signed
        result.broadcasted = true
        result.persistenceError = "fixture storage failure"
        let broadcast = result
        service.broadcastReply = { broadcast }
        service.refreshReply = { throw BsvHomeServiceError.Store(msg: "fixture refresh failure") }
        var snapshot = BsvColdWalletServiceMock.fixture
        snapshot.wallet = try await service.installScopes()
        snapshot.role = .observer
        let model = BsvColdWalletViewModel(service: service, mode: .online)
        model.snapshot = snapshot
        model.pending = BsvColdWalletServiceMock.signed
        #expect(model.canBroadcast)
        let navigation = NavigationPathState()
        navigation.append(BsvColdWalletRoute.broadcast)
        model.broadcast(navigation: navigation)
        await model.operationTask?.value
        #expect(model.path == [.result])
        #expect(model.pending?.broadcasted == true)
        #expect(model.pending?.persistenceError != nil)
        #expect(model.pending?.transactionId == broadcast.transactionId)
        #expect(!model.canBroadcast)
    }

    @Test
    func reappearingAfterInternalNavigationDoesNotReopenTheInitialEntry() async {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.snapshotReply = { BsvColdWalletServiceMock.observerFixture }
        let model = BsvColdWalletViewModel(service: service, mode: .online, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: .send)
        await model.load()
        #expect(model.path == [.send])
        model.path = []
        await model.load()
        #expect(model.path.isEmpty)
    }

    @Test(arguments: [false, true])
    func failedImportClearsTemporarySecretsUnlessAuthenticationWasCancelled(cancelled: Bool) async {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.importReply = { throw cancelled ? BsvHomeServiceError.Cancelled : BsvHomeServiceError.InvalidInput(msg: "fixture") }
        let model = BsvColdWalletViewModel(service: service, mode: .offline)
        model.walletName = "Fixture"
        model.input = "public fixture"
        model.passphrase = "fixture passphrase"
        model.previewImport()
        await model.operationTask?.value
        model.isPreviewConfirmed = true
        model.confirmImport()
        await model.operationTask?.value
        #expect(model.isPreviewConfirmed == cancelled)
        #expect(model.input.isEmpty != cancelled)
        #expect(model.passphrase.isEmpty != cancelled)
        #expect(model.path.last == (cancelled ? .importReview : .recover))
    }

    @Test
    func editingImportConfigurationInvalidatesThePreviewAcknowledgment() async {
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .offline)
        model.walletName = "Fixture"
        model.input = "public fixture"
        model.previewImport()
        await model.operationTask?.value
        #expect(model.path.last == .importReview)
        model.isPreviewConfirmed = true
        model.configuration.customPath = "m/44'/145'/0'"
        #expect(!model.isPreviewConfirmed)
        model.isPreviewConfirmed = true
        model.passphrase = "changed fixture"
        #expect(!model.isPreviewConfirmed)
    }

    @Test
    func selectingAnotherWalletClearsAcknowledgmentsAndPublicFormContext() async {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        var next = BsvColdWalletServiceMock.fixture
        next.wallet.id = "another-fixture-wallet"
        let selected = next
        service.candidatesReply = { [
            .init(wallet: BsvColdWalletServiceMock.fixture.wallet, role: BsvColdWalletServiceMock.fixture.role),
            .init(wallet: selected.wallet, role: selected.role),
        ] }
        let model = BsvColdWalletViewModel(service: service, mode: .offline)
        await model.load()
        service.snapshotReply = { selected }
        model.isPreviewConfirmed = true
        model.isReviewConfirmed = true
        model.input = "fixture recovery"
        model.passphrase = "fixture passphrase"
        model.recipient = "previous address"
        model.amount = "1"
        model.selectWallet(selected.wallet.id)
        await model.operationTask?.value
        #expect(model.wallet?.id == selected.wallet.id)
        #expect(!model.isPreviewConfirmed)
        #expect(!model.isReviewConfirmed)
        #expect(model.input.isEmpty)
        #expect(model.passphrase.isEmpty)
        #expect(model.recipient.isEmpty)
        #expect(model.amount.isEmpty)
    }

    @Test(arguments: [false, true])
    func lateCreationCannotPublishAfterBackgroundOrCapture(captured: Bool) async {
        let reply = BsvColdWalletAsyncReply<BsvHomeBsvColdWalletSnapshot>()
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.createReply = { try await reply.wait() }
        let model = BsvColdWalletViewModel(service: service, mode: .offline)
        model.walletName = "Fixture"
        model.words = ["public", "fixture"]
        model.input = "fixture input"
        model.passphrase = "fixture passphrase"
        model.backup = "fixture backup"
        model.isBackupConfirmed = true
        model.create()
        let operation = model.operationTask
        await reply.waitForEntry()
        if captured {
            model.captureChanged(true)
        } else {
            model.sceneChanged(.background)
        }
        await reply.complete(BsvColdWalletServiceMock.fixture)
        await operation?.value
        #expect(model.snapshot?.wallet == nil)
        #expect(model.words.isEmpty)
        #expect(model.input.isEmpty)
        #expect(model.passphrase.isEmpty)
        #expect(model.backup == nil)
        #expect(model.path.isEmpty)
    }

    @Test
    func repeatedCreationIsRejectedWhileAuthenticationOrImportIsPending() async {
        let reply = BsvColdWalletAsyncReply<BsvHomeBsvColdWalletSnapshot>()
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.createReply = { try await reply.wait() }
        let model = BsvColdWalletViewModel(service: service, mode: .offline, protectStorage: { throw BsvHomeServiceError.Store(msg: "fixture") })
        model.walletName = "Fixture"
        model.words = ["public", "fixture"]
        model.isBackupConfirmed = true
        model.create()
        let operation = model.operationTask
        await reply.waitForEntry()
        model.create()
        #expect(await service.calls.creates == 1)
        await reply.complete(BsvColdWalletServiceMock.fixture)
        await operation?.value
        #expect(!model.isBusy)
        #expect(await service.calls.creates == 1)
    }

    @Test
    func incompleteQrKeepsScanningAndConflictClearsTheSession() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        let model = BsvColdWalletViewModel(service: service, mode: .offline)
        let first = "{\"kind\":\"wowpay-bsv-qr\",\"index\":0}"
        let second = "{\"kind\":\"wowpay-bsv-qr\",\"index\":1}"
        model.snapshot = BsvColdWalletServiceMock.fixture
        model.startScan(.request)
        try model.scan(first, scannerId: #require(model.scannerSession?.id))
        await model.operationTask?.value
        #expect(model.scanner == .request)
        #expect(model.receivedFrames == 1)
        #expect(model.errorMessage == nil)
        service.assembleReply = { _ in throw BsvHomeServiceError.InvalidInput(msg: "fixture conflict") }
        try model.scan(second, scannerId: #require(model.scannerSession?.id))
        await model.operationTask?.value
        #expect(model.scanner == .request)
        #expect(model.receivedFrames == 0)
        #expect(model.errorMessage != nil)
        service.assembleReply = { _ in nil }
        try model.scan(first, scannerId: #require(model.scannerSession?.id))
        await model.operationTask?.value
        #expect(model.receivedFrames == 1)
    }
}
