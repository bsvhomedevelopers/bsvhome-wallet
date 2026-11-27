@testable import BsvColdWallet
import Components
import Foundation
import BsvHomeCore
import BsvHomeCoreServicesTestKit
import SwiftUI
import Testing

@MainActor
struct BsvColdWalletScanRequestNavigationTests {
    @Test
    func initialEntryCreatesOneScannerOverTheRealSigningWorkspaceWithoutExportingAnotherIdentity() async throws {
        let (model, service) = await loadEntryModel(entry: .scanRequest(presentation: .initial))
        #expect(model.path == [.workspaceHome, .signing])
        #expect(model.scannerSession == nil)
        model.activateInitialRequestScanner()
        #expect(model.scannerSession?.kind == .request)
        #expect(model.scannerSession?.origin == .initialEntry)
        #expect(await service.calls.pairings.isEmpty)
        let identifier = try #require(model.scannerSession?.id)
        await model.load()
        #expect(model.scannerSession?.id == identifier)
    }

    @Test
    func resumedEntryDoesNotReplayTheInitialScanner() async {
        let (model, _) = await loadEntryModel(entry: .scanRequest(presentation: .resumed))
        #expect(model.path == [.workspaceHome, .signing])
        #expect(model.scannerSession == nil)
        model.startScan(.request)
        #expect(model.scannerSession?.origin == .workspace)
    }

    @Test
    func initialScannerCancellationRestoresOnlyTheCapturedParentAndNeverCompletes() async throws {
        var completed = 0
        let (model, _) = await loadEntryModel(entry: .scanRequest(presentation: .initial), onComplete: { completed += 1 })
        let owner = NavigationPathState()
        owner.append("parent")
        let parent = owner.path
        owner.append("scan-request-entry")
        let entry = owner.path
        let navigation = BsvColdWalletNavigation(navigation: owner)
        navigation.show(model.path, model: model)
        model.activateInitialRequestScanner()
        let identifier = try #require(model.scannerSession?.id)
        try #require(model.closeScanner(identifier) == .initialEntry)
        var cancelled = 0
        navigation.cancelInitialScanner(model: model) {
            #expect(owner.path == entry)
            cancelled += 1
            owner.removeLast()
        }
        #expect(owner.path == parent)
        #expect(model.isCancelled)
        #expect(cancelled == 1)
        #expect(completed == 0)
        navigation.cancelInitialScanner(model: model) { cancelled += 1 }
        #expect(cancelled == 1)
    }

    @Test
    func workspaceCancellationAndStaleSheetNilCannotCloseTheEntryOrANewerScanner() async throws {
        let (model, _) = await loadEntryModel(entry: .scanRequest(presentation: .resumed))
        model.startScan(.request)
        let previous = try #require(model.scannerSession?.id)
        #expect(model.closeScanner(previous) == .workspace)
        #expect(model.path == [.workspaceHome, .signing])
        #expect(!model.isCancelled)
        model.startScan(.request)
        let current = try #require(model.scannerSession?.id)
        #expect(current != previous)
        #expect(model.closeScanner(previous) == nil)
        #expect(model.scannerSession?.id == current)
    }

    @Test(arguments: [BsvColdWalletEntry.signing, .scanRequest(presentation: .resumed)])
    func theActualScanProducerClosesItsMatchingSessionBeforeReviewAndBackReturnsToSigning(entry: BsvColdWalletEntry) async throws {
        let (model, service) = await loadEntryModel(entry: entry)
        var pending = BsvColdWalletServiceMock.signed
        pending.signedJson = nil
        pending.transactionId = nil
        let result = pending
        service.importRequestReply = { result }
        let prefix: [BsvColdWalletRoute] = entry == .signing ? [.signing] : [.workspaceHome, .signing]
        let owner = NavigationPathState()
        owner.append("entry")
        let navigation = BsvColdWalletNavigation(navigation: owner)
        navigation.show(model.path, model: model)
        model.startScan(.request)
        let identifier = try #require(model.scannerSession?.id)
        model.scan("{\"kind\":\"wowpay-bsv-request\"}", scannerId: identifier)
        let operation = try #require(model.operationTask)
        await operation.value
        #expect(model.path == prefix + [.review])
        #expect(model.scannerSession == nil)
        #expect(model.closeScanner(identifier) == nil)
        navigation.show(model.path, model: model)
        owner.removeLast()
        navigation.reconcile(model: model)
        #expect(model.path == prefix)
        #expect(!model.isCancelled)
    }

    @Test(.timeLimit(.minutes(1)), arguments: [BsvColdWalletEntry.signing, .scanRequest(presentation: .resumed)])
    func aRealCoreScopeSignedResultRetainsSigningAsItsActualBackDestination(entry: BsvColdWalletEntry) async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        let vector = try JSONSerialization.jsonObject(with: BsvColdWalletSigningScopeFixture.signingVectorData()) as? [String: Any]
        let signed = try #require(vector?["signed"] as? [String: Any])
        let signedJson = try #require(String(data: JSONSerialization.data(withJSONObject: signed, options: [.sortedKeys]), encoding: .utf8))
        let wallet = try await service.installScopes(onSigning: { _, _, _, _ in signedJson })
        var initial = BsvColdWalletServiceMock.fixture
        initial.wallet = wallet
        initial.pending = service.fixturePending
        let snapshot = initial
        service.snapshotReply = { snapshot }
        service.exportPairingReply = { _, _ in snapshot }
        service.candidatesReply = { [.init(wallet: wallet, role: .signer)] }
        service.currentWalletIdReply = { wallet.id }
        service.signWithScopeReply = { [weak service] scope in
            guard let service else { throw BsvHomeServiceError.Cancelled }
            return try await service.executeFixtureSign(scope: scope)
        }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: wallet.id, initialPage: entry)
        await model.load()
        let prefix: [BsvColdWalletRoute] = entry == .signing ? [.signing] : [.workspaceHome, .signing]
        model.path = prefix + [.review]
        model.isReviewConfirmed = true
        let owner = NavigationPathState()
        owner.append("entry")
        let navigation = BsvColdWalletNavigation(navigation: owner)
        navigation.show(model.path, model: model)
        model.sign(navigation: owner)
        let operation = try #require(model.operationTask)
        await operation.value
        #expect(model.pending?.signedJson != nil)
        #expect(model.path == prefix + [.signed])
        navigation.show(model.path, model: model)
        owner.removeLast()
        navigation.reconcile(model: model)
        #expect(model.path == prefix)
        #expect(!model.isCancelled)
    }

    @Test
    func scanRequestSigningBackShowsTheRealColdHomeThenClosesOnlyItsEntry() async {
        let (model, _) = await loadEntryModel(entry: .scanRequest(presentation: .resumed))
        let owner = NavigationPathState()
        owner.append("parent")
        let parent = owner.path
        owner.append("entry")
        let navigation = BsvColdWalletNavigation(navigation: owner)
        navigation.show(model.path, model: model)
        var cancelled = 0
        let close: @MainActor () -> Void = { cancelled += 1; owner.removeLast() }
        owner.removeLast()
        navigation.reconcile(model: model, onInitialPageBack: close)
        #expect(model.path == [.workspaceHome])
        #expect(cancelled == 0)
        owner.removeLast()
        navigation.reconcile(model: model, onInitialPageBack: close)
        #expect(model.isCancelled)
        #expect(owner.path == parent)
        #expect(cancelled == 1)
    }

    @Test
    func initialCancelNeverRewritesAForeignPathOrInvokesItsParentCallback() async {
        let (model, _) = await loadEntryModel(entry: .scanRequest(presentation: .resumed))
        let owner = NavigationPathState()
        owner.append("entry")
        let navigation = BsvColdWalletNavigation(navigation: owner)
        navigation.show(model.path, model: model)
        owner.setPath(["foreign"])
        let foreign = owner.path
        var cancelled = 0
        navigation.cancelInitialScanner(model: model) { cancelled += 1 }
        #expect(model.isCancelled)
        #expect(owner.path == foreign)
        #expect(cancelled == 0)
    }

    @Test
    func aReentrantRetirementCannotCloseTheIncomingPath() async {
        let (model, _) = await loadEntryModel(entry: .scanRequest(presentation: .resumed))
        let owner = NavigationPathState()
        owner.append("entry")
        let navigation = BsvColdWalletNavigation(navigation: owner)
        navigation.show(model.path, model: model)
        var incoming = owner.path
        incoming.append("reentrant")
        owner.observeRemoval { owner.append("reentrant") }
        var cancelled = 0
        navigation.cancelInitialScanner(model: model) { cancelled += 1 }
        #expect(owner.path == incoming)
        #expect(model.isCancelled)
        #expect(cancelled == 0)
    }

    @Test
    func dashboardHandoffUsesItsActualSnapshotWalletAndDoesNotOpenALocalSigningScanner() async throws {
        let (model, _) = await loadEntryModel(entry: .home)
        var selected = try #require(model.snapshot)
        selected.wallet.id = "public-wallet-B"
        model.snapshot = selected
        var opened: [String] = []
        model.requestSigning { opened.append($0) }
        #expect(opened == ["public-wallet-B"])
        #expect(model.scannerSession == nil)
        model.cancel()
        model.requestSigning { opened.append($0) }
        #expect(opened.count == 1)
    }

    @Test
    func dashboardPairingHandoffUsesItsActualSnapshotWalletWithoutStartingALocalExport() async throws {
        let (model, service) = await loadEntryModel(entry: .home)
        var selected = try #require(model.snapshot)
        selected.wallet.id = "public-wallet-B"
        model.snapshot = selected
        var opened: [String] = []
        model.requestPairing { opened.append($0) }
        #expect(opened == ["public-wallet-B"])
        #expect(model.operationTask == nil)
        #expect(model.path.isEmpty)
        #expect(model.frames.isEmpty)
        #expect(await service.calls.pairings.isEmpty)
    }

    @Test
    func anInactiveCapturedOrCancelledDashboardCannotHandoffPairing() async {
        let (model, service) = await loadEntryModel(entry: .home)
        var opened: [String] = []
        model.sceneChanged(.inactive)
        model.requestPairing { opened.append($0) }
        model.sceneChanged(.active)
        model.captureChanged(true)
        model.requestPairing { opened.append($0) }
        model.cancel()
        model.requestPairing { opened.append($0) }
        #expect(opened.isEmpty)
        #expect(await service.calls.pairings.isEmpty)
    }

    @Test
    func anObserverDashboardCannotHandoffAKeyHoldingPairingEntry() async throws {
        let (model, service) = await loadEntryModel(entry: .home)
        var selected = try #require(model.snapshot)
        selected.role = .observer
        model.snapshot = selected
        var opened: [String] = []
        model.requestPairing { opened.append($0) }
        #expect(opened.isEmpty)
        #expect(model.operationTask == nil)
        #expect(await service.calls.pairings.isEmpty)
    }

    @Test
    func selectingWalletBPreservesTheScanRequestSigningWorkspaceWithoutReplayingTheScanner() async throws {
        let first = BsvColdWalletServiceMock.fixture
        var second = first
        second.wallet.id = "public-wallet-B"
        let cursor = BsvColdWalletServiceMock.SnapshotCursor(first: first, second: second)
        let service = BsvColdWalletServiceMock(noHandle: .init())
        let candidates: [BsvHomeBsvColdWalletCandidate] = [.init(wallet: first.wallet, role: .signer), .init(wallet: second.wallet, role: .signer)]
        service.candidatesReply = { candidates }
        service.snapshotReply = { cursor.snapshot() }
        service.currentWalletIdReply = { cursor.snapshot().wallet.id }
        service.selectWalletReply = { try cursor.select($0) }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: first.wallet.id, initialPage: .scanRequest(presentation: .resumed))
        await model.load()
        model.selectWallet(second.wallet.id)
        let selection = try #require(model.operationTask)
        await selection.value
        #expect(model.wallet?.id == second.wallet.id)
        #expect(model.path == [.workspaceHome, .signing])
        #expect(model.scannerSession == nil)
    }

    @Test
    func legacyHomeKeepsItsLocalScannerAndOriginalRootPath() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.currentWalletIdReply = { BsvColdWalletServiceMock.fixture.wallet.id }
        let model = BsvColdWalletViewModel(service: service, mode: .offline, walletId: BsvColdWalletServiceMock.fixture.wallet.id)
        await model.load()
        model.startScan(.request)
        let identifier = try #require(model.scannerSession?.id)
        #expect(model.scannerSession?.origin == .workspace)
        #expect(model.closeScanner(identifier) == .workspace)
        #expect(model.path.isEmpty)
        #expect(!model.isCancelled)
    }

    @Test
    func pendingInitialWorkspaceDoesNotCountAsTheRenderedSigningPage() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.currentWalletIdReply = { BsvColdWalletServiceMock.fixture.wallet.id }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: .scanRequest(presentation: .initial))
        let owner = NavigationPathState()
        owner.append("entry")
        let entry = owner.path
        let navigation = BsvColdWalletNavigation(navigation: owner)
        navigation.appear(model: model)
        #expect(owner.path == entry)
        await model.load()
        try #require(model.path == [.workspaceHome, .signing])
        #expect(!navigation.isShowing(.signing, model: model))
        if navigation.isShowing(.signing, model: model) {
            model.activateInitialRequestScanner()
        }
        #expect(model.scannerSession == nil)
        #expect(owner.path == entry)
        navigation.show(model.path, model: model)
        #expect(navigation.isShowing(.signing, model: model))
        model.activateInitialRequestScanner()
        let scanner = try #require(model.scannerSession)
        #expect(scanner.origin == .initialEntry)
        model.path = [.workspaceHome, .review]
        #expect(!navigation.isShowing(.review, model: model))
        navigation.show(model.path, model: model)
        #expect(navigation.isShowing(.review, model: model))
        model.cancel()
    }

    private func loadEntryModel(entry: BsvColdWalletEntry, onComplete: @escaping @MainActor () -> Void = {}) async -> (BsvColdWalletViewModel, BsvColdWalletServiceMock) {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.currentWalletIdReply = { BsvColdWalletServiceMock.fixture.wallet.id }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: entry, onComplete: onComplete)
        await model.load()
        return (model, service)
    }
}
