@testable import BsvColdWallet
import Components
import Foundation
import BsvHomeCore
import SwiftUI
import Testing

@MainActor
struct BsvColdWalletUnifiedTests {
    @Test
    func unifiedDashboardUsesActualRoleRatherThanWalletType() {
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .unified)
        var signer = BsvColdWalletServiceMock.fixture
        signer.wallet.walletType = .view
        signer.role = .signer
        model.snapshot = signer
        #expect(model.isSigner)
        #expect(!model.isObserver)
        #expect(!model.canPrepare)
        model.pending = BsvColdWalletServiceMock.signed
        #expect(!model.canBroadcast)
        var observer = BsvColdWalletServiceMock.fixture
        observer.role = .observer
        model.snapshot = observer
        #expect(model.isObserver)
        #expect(!model.canSign)
        #expect(model.canBroadcast)
    }

    @Test(arguments: [BsvColdWalletEntry.create, .recover, .connect, .signing])
    func initialUnifiedEntryIsAppliedOnceAndInternalReappearancePreservesThePage(entry: BsvColdWalletEntry) async {
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .unified, initialPage: entry)
        await model.load()
        let initial = model.path
        #expect(initial.count == 1)
        model.path.append(.importReview)
        await model.load()
        #expect(model.path.count == 2)
        #expect(model.path.first == initial.first)
    }

    @Test
    func sameKindScannerCallbackFromOldPresentationCannotEnterTheNewSession() async throws {
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .unified)
        model.snapshot = BsvColdWalletServiceMock.fixture
        model.startScan(.request)
        let previous = try #require(model.scannerSession?.id)
        model.closeScanner(previous)
        model.startScan(.request)
        let current = try #require(model.scannerSession?.id)
        model.scan("{\"kind\":\"wowpay-bsv-qr\"}", scannerId: previous)
        #expect(model.operationTask == nil)
        #expect(model.scannerSession?.id == current)
        #expect(model.receivedFrames == 0)
        model.scan("{\"kind\":\"wowpay-bsv-qr\"}", scannerId: current)
        await model.operationTask?.value
        #expect(model.scannerSession?.id == current)
        #expect(model.receivedFrames == 1)
    }

    @Test
    func recipientScannerUsesTheRealCoreDecoderAndPreservesExactAmountWithoutAcceptingMemo() {
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .unified)
        model.snapshot = BsvColdWalletServiceMock.observerFixture
        model.path = [.send]
        model.amount = "1"
        var reply: (@MainActor (String) -> Void)?
        model.scanRecipient { reply = $0 }
        reply?("bitcoinsv:1BitcoinEaterAddressDontSendf59kuE?amount=0.00000001")
        #expect(model.recipient == "1BitcoinEaterAddressDontSendf59kuE")
        #expect(BsvColdWalletAmount.satoshis(model.amount, decimalSeparator: Locale.current.decimalSeparator ?? ".") == 1)
        let amount = model.amount
        model.scanRecipient { reply = $0 }
        reply?("bitcoinsv:1BitcoinEaterAddressDontSendf59kuE?amount=2&memo=note")
        #expect(model.amount == amount)
        #expect(model.errorMessage != nil)
        model.scanRecipient { reply = $0 }
        model.cancelActions()
        reply?("bitcoinsv:1BitcoinEaterAddressDontSendf59kuE?amount=2")
        #expect(model.amount == amount)
    }

    @Test
    func retiringNavigationRemovesOnlyItsOwnTypedSuffixAndDoesNotOverwriteAReplacement() {
        let path = NavigationPathState()
        path.binding.wrappedValue.append("caller")
        let entry = path.path
        let navigation = BsvColdWalletNavigation(navigation: path)
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .unified)
        model.attachNavigationRetirement { navigation.retire() }
        navigation.show([.recover, .importReview], model: model)
        #expect(path.path.count == entry.count + 2)
        model.cancel()
        #expect(path.path == entry)

        let old = BsvColdWalletNavigation(navigation: path)
        let previous = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .unified)
        previous.attachNavigationRetirement { old.retire() }
        old.show([.send], model: previous)
        var replacement = NavigationPath()
        replacement.append("replacement")
        path.binding.wrappedValue = replacement
        previous.cancel()
        #expect(path.path == replacement)
    }

    @Test
    func parentCancellationClearsTheMatchingAuthorizationAndRejectsLateBroadcastProjection() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        let wallet = try await service.installScopes()
        let reply = BsvColdWalletAsyncReply<BsvHomeBsvColdWalletPending>()
        service.broadcastReply = { try await reply.wait() }
        var cancelled: [String] = []
        let model = BsvColdWalletViewModel(service: service, mode: .unified, cancelAuthorization: { cancelled.append($0) })
        var snapshot = BsvColdWalletServiceMock.observerFixture
        snapshot.wallet = wallet
        model.snapshot = snapshot
        model.pending = BsvColdWalletServiceMock.signed
        let navigation = NavigationPathState()
        navigation.append(BsvColdWalletRoute.broadcast)
        model.broadcast(navigation: navigation)
        let task = try #require(model.operationTask)
        await reply.waitForEntry()
        let scope = try #require(service.lastScope)
        model.cancelActions()
        var accepted = BsvColdWalletServiceMock.signed
        accepted.broadcasted = true
        await reply.complete(accepted)
        await task.value
        #expect(cancelled == [scope.id()])
        #expect(model.pending?.broadcasted == false)
        #expect(model.path.isEmpty)
        #expect(!model.isBusy)
        #expect(service.invalidations == 1)
    }
}
