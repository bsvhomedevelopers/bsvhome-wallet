@testable import BsvColdWallet
import BsvHomeCore
import Testing

@MainActor
struct BsvColdWalletPairingEntryTests {
    @Test(arguments: [BsvColdWalletEntry.pairing, .signing])
    func explicitPreferredEntryUsesTheCoreExportEvenWhenOrdinaryWalletIsNotAColdCandidate(entry: BsvColdWalletEntry) async {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [] }
        let snapshot = BsvColdWalletServiceMock.fixture
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: snapshot.wallet.id, initialPage: entry)
        await model.load()
        #expect(await service.calls.pairings == [snapshot.wallet.id])
        #expect(await service.calls.pairingGenerations == [1])
        #expect(model.snapshot?.wallet.id == snapshot.wallet.id)
        #expect(model.candidates.map(\.wallet.id) == [snapshot.wallet.id])
        #expect(model.isSigner)
        #expect(model.path == (entry == .pairing ? [.pairing] : [.signing]))
        #expect(model.frames == (entry == .pairing ? [snapshot.pairingJson] : []))
        #expect(!model.loadFailed)
    }

    @Test
    func ordinaryHomeLoadingDoesNotConvertAPreferredWalletOrInventAColdCandidate() async {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [] }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: BsvColdWalletServiceMock.fixture.wallet.id)
        await model.load()
        #expect(await service.calls.pairings.isEmpty)
        #expect(model.snapshot == nil)
        #expect(model.candidates.isEmpty)
        #expect(model.frames.isEmpty)
        #expect(model.path.isEmpty)
    }

    @Test(arguments: [BsvColdWalletEntry.pairing, .signing])
    func aFailedExplicitEntryDoesNotFallBackToTheSigningChooserOrEmitQRFrames(entry: BsvColdWalletEntry) async {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [] }
        service.exportPairingReply = { _, _ in throw BsvHomeServiceError.Store(msg: "Public pairing fixture failure") }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: entry)
        await model.load()
        #expect(await service.calls.pairings.count == 1)
        #expect(model.snapshot == nil)
        #expect(model.frames.isEmpty)
        #expect(model.path.isEmpty)
        #expect(model.loadFailed)
        #expect(model.errorMessage != nil)
    }

    @Test(arguments: [BsvColdWalletEntry.pairing, .signing])
    func cancellationRejectsALateExportedSnapshotWithoutReplacingTheWalletOrShowingAPairingCode(entry: BsvColdWalletEntry) async {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [] }
        let reply = BsvColdWalletAsyncReply<BsvHomeBsvColdWalletSnapshot>()
        service.exportPairingReply = { _, _ in try await reply.wait() }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: entry)
        let loading = Task { await model.load() }
        await reply.waitForEntry()
        model.cancelActions()
        await reply.complete(BsvColdWalletServiceMock.fixture)
        await loading.value
        #expect(await service.calls.pairings.count == 1)
        #expect(service.invalidations == 1)
        #expect(model.snapshot == nil)
        #expect(model.frames.isEmpty)
        #expect(model.path.isEmpty)
    }

    @Test(arguments: [BsvColdWalletEntry.pairing, .signing])
    func anExplicitEntryCancelledBeforeLoadingNeverEntersTheCoreExport(entry: BsvColdWalletEntry) async {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.candidatesReply = { [] }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: entry)
        let loading = Task { await model.load() }
        model.cancel()
        await loading.value
        #expect(await service.calls.pairings.isEmpty)
        #expect(model.snapshot == nil)
        #expect(model.frames.isEmpty)
        #expect(model.path.isEmpty)
    }

    @Test
    func explicitPairingCancelledBeforeTheTaskFirstPollNeverEntersTheCoreOwner() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        let model = BsvColdWalletViewModel(service: service, mode: .unified)
        var ordinary = BsvColdWalletServiceMock.fixture
        ordinary.role = .none
        model.snapshot = ordinary
        model.exportPairing()
        let task = try #require(model.operationTask)
        model.cancelActions()
        await task.value
        #expect(await service.calls.pairings.isEmpty)
        #expect(model.frames.isEmpty)
        #expect(model.path.isEmpty)
    }
}
