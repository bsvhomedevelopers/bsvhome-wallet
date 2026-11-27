@testable import BsvColdWallet
import Components
import BsvHomeCore
import SwiftUI
import Testing

@MainActor
struct BsvColdWalletPublicResumeTests {
    @Test
    func aLoadedResumedSigningWorkspaceReturnsAfterBackgroundWithoutReopeningTheScanner() async throws {
        let model = makeModel(entry: .scanRequest(presentation: .resumed))
        await model.load()
        try #require(model.path == [.workspaceHome, .signing])
        try #require(model.snapshot?.role == .signer)
        model.sceneChanged(.background)
        #expect(model.scannerSession == nil)
        model.sceneChanged(.active)
        let reload = try #require(model.operationTask)
        await reload.value
        try #require(model.snapshot?.role == .signer)
        #expect(!model.isBusy)
        #expect(model.path == [.workspaceHome, .signing])
        model.activateInitialRequestScanner()
        #expect(model.scannerSession == nil)
    }

    @Test
    func aResumedEntryBackgroundedBeforeItsFirstLoadStillEntersOnlyThePublicSigningWorkspace() async throws {
        let model = makeModel(entry: .scanRequest(presentation: .resumed))
        model.sceneChanged(.background)
        model.sceneChanged(.active)
        let load = try #require(model.operationTask)
        await load.value
        try #require(model.snapshot?.role == .signer)
        #expect(!model.isBusy)
        #expect(model.path == [.workspaceHome, .signing])
        model.activateInitialRequestScanner()
        #expect(model.scannerSession == nil)
        #expect(model.words.isEmpty)
        #expect(model.backup == nil)
    }

    @Test
    func aUserWhoReturnedToPublicHomeIsNotPulledBackIntoSigningAfterBackground() async throws {
        let model = makeModel(entry: .scanRequest(presentation: .resumed))
        await model.load()
        try #require(model.path == [.workspaceHome, .signing])
        let owner = NavigationPathState()
        owner.append("public-entry")
        let navigation = model.navigation(in: owner)
        navigation.show(model.path, model: model)
        owner.removeLast()
        navigation.reconcile(model: model)
        try #require(model.path == [.workspaceHome])
        model.sceneChanged(.background)
        model.sceneChanged(.active)
        let reload = try #require(model.operationTask)
        await reload.value
        try #require(model.snapshot?.role == .signer)
        #expect(model.path == [.workspaceHome])
        model.activateInitialRequestScanner()
        #expect(model.scannerSession == nil)
    }

    @Test(arguments: [BsvColdWalletEntry.create, .recover])
    func secretEntriesBackgroundedBeforeTheirFirstLoadDoNotReplayTheirInitialAction(entry: BsvColdWalletEntry) async throws {
        let model = makeModel(entry: entry)
        model.sceneChanged(.background)
        model.sceneChanged(.active)
        let load = try #require(model.operationTask)
        await load.value
        try #require(model.snapshot?.role == .signer)
        #expect(model.path.isEmpty)
        #expect(model.words.isEmpty)
        #expect(model.backup == nil)
        #expect(model.scannerSession == nil)
    }

    private func makeModel(entry: BsvColdWalletEntry) -> BsvColdWalletViewModel {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.currentWalletIdReply = { BsvColdWalletServiceMock.fixture.wallet.id }
        return BsvColdWalletViewModel(service: service, mode: .unified, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: entry)
    }
}
