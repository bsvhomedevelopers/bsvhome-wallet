import BsvMultisig
import Components
import Foundation
@testable import BsvHome
import BsvHomeCore
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import Primitives
import Testing

@MainActor
struct BsvPublicFundingContextTests {
    @Test
    func clearingAnUnavailablePayerKeepsTheOriginalPaymailSourceUntilTheContextExits() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let payer = fixture.wallet.map()
        let option = option(payer)
        let navigation = NavigationPathState()
        navigation.append("previous-page")
        var unavailable = false
        var created: [Primitives.Wallet] = []
        let context = context(fixture: fixture, preparation: preparation, navigation: navigation, loadCandidates: {
            if unavailable {
                throw CandidateFailure.unavailable
            }
            return ([payer], [option])
        }, onMakeModel: { created.append($0) })
        context.present(onExit: {})
        context.loadPayers()
        try await (#require(context.loadTask)).value
        #expect(context.model == nil)
        #expect(created.isEmpty)
        context.selectPayer(option)
        #expect(context.model?.payer.id == payer.id.id)
        #expect(created == [payer])
        let entry = navigation.path
        navigation.append("funding-review")
        unavailable = true
        context.loadPayers()
        try await (#require(context.loadTask)).value
        #expect(context.model == nil)
        #expect(context.eligiblePayers.isError)
        #expect(navigation.path == entry)
        let confirmed = try await fixture.service.confirmPublicFunding(preparation: preparation, value: 84000)
        #expect(confirmed.amount == "0.00084")
        context.dismiss()
        #expect(navigation.count == 1)
        await expectRetired(preparation, fixture: fixture)
        #expect(fixture.credentialAccessCount == 0)
        #expect(fixture.rejectedRequestCount == 0)
    }

    @Test
    func internalNavigationKeepsTheSourceAndTruePrefixRemovalRetiresItExactlyOnce() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let navigation = NavigationPathState()
        navigation.append("previous-page")
        var exits = 0
        let context = context(fixture: fixture, preparation: preparation, navigation: navigation)
        context.present { exits += 1 }
        navigation.append("funding-review")
        navigation.removeLast()
        let confirmed = try await fixture.service.confirmPublicFunding(preparation: preparation, value: 42000)
        #expect(confirmed.amount == "0.00042")
        #expect(exits == 0)
        navigation.removeLast()
        #expect(exits == 1)
        await expectRetired(preparation, fixture: fixture)
        context.dismiss()
        #expect(exits == 1)
        #expect(navigation.count == 1)
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test
    func dismissingAnOldContextCannotRemoveTheNewSourceOrItsRoute() async throws {
        let firstFixture = try await SendPublicPaymailFixture.make()
        let nextFixture = try await SendPublicPaymailFixture.make()
        let firstPreparation = try await firstFixture.prepare(value: 42000)
        let nextPreparation = try await nextFixture.prepare(value: 84000)
        let navigation = NavigationPathState()
        navigation.append("previous-page")
        var firstExits = 0
        var nextExits = 0
        let first = context(fixture: firstFixture, preparation: firstPreparation, navigation: navigation)
        first.present { firstExits += 1 }
        first.dismiss()
        let next = context(fixture: nextFixture, preparation: nextPreparation, navigation: navigation)
        next.present { nextExits += 1 }
        let nextPath = navigation.path
        first.dismiss()
        #expect(navigation.path == nextPath)
        #expect(firstExits == 1)
        #expect(nextExits == 0)
        await expectRetired(firstPreparation, fixture: firstFixture)
        let confirmed = try await nextFixture.service.confirmPublicFunding(preparation: nextPreparation, value: 84000)
        #expect(confirmed.amount == "0.00084")
        next.dismiss()
        #expect(nextExits == 1)
        #expect(navigation.count == 1)
        await expectRetired(nextPreparation, fixture: nextFixture)
        #expect(firstFixture.credentialAccessCount == 0)
        #expect(nextFixture.credentialAccessCount == 0)
    }

    @Test
    func dismissalBeforeTheFirstCandidatePollPerformsNoCandidateIO() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let navigation = NavigationPathState()
        var reads = 0
        let context = context(fixture: fixture, preparation: preparation, navigation: navigation, loadCandidates: {
            reads += 1
            return ([], [])
        })
        context.present(onExit: {})
        context.loadPayers()
        let task = try #require(context.loadTask)
        context.dismiss()
        await task.value
        #expect(reads == 0)
        #expect(context.loadTask == nil)
        #expect(context.model == nil)
        #expect(navigation.isEmpty)
        await expectRetired(preparation, fixture: fixture)
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test(.timeLimit(.minutes(1)))
    func aLateCandidateReadCannotReviveTheRetiredContext() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let payer = fixture.wallet.map()
        let option = option(payer)
        let navigation = NavigationPathState()
        let gate = CandidateGate()
        let context = context(fixture: fixture, preparation: preparation, navigation: navigation, loadCandidates: {
            await gate.wait()
            return ([payer], [option])
        })
        context.present(onExit: {})
        context.loadPayers()
        let task = try #require(context.loadTask)
        await gate.started()
        context.dismiss()
        await gate.release()
        await task.value
        #expect(context.loadTask == nil)
        #expect(context.model == nil)
        #expect(context.eligiblePayers.value == nil)
        context.selectPayer(option)
        #expect(context.model == nil)
        await expectRetired(preparation, fixture: fixture)
        #expect(fixture.credentialAccessCount == 0)
    }

    private func context(
        fixture: SendPublicPaymailFixture,
        preparation: BsvHomeSendPublicFundingPreparation,
        navigation: NavigationPathState,
        loadCandidates: @escaping @MainActor () async throws -> (wallets: [Primitives.Wallet], payers: [BsvHomeBsvMultisigFundingPayer]) = { ([], []) },
        onMakeModel: @escaping @MainActor (Primitives.Wallet) -> Void = { _ in },
    ) -> BsvPublicFundingContext {
        let entry = BsvMultisigPublicFundingEntry.payment(.paymail(service: fixture.service, preparation: preparation))
        let authorization = BsvMultisigPasswordAuthorization(makeService: { _ in BsvHomePortalTransferPasswordServiceMock() }, isForeground: { true })
        return BsvPublicFundingContext(entry: entry, navigation: navigation, authorization: authorization, loadCandidates: loadCandidates, makeModel: { wallet in
            onMakeModel(wallet)
            return BsvMultisigPublicFundingViewModel(entry: entry, payer: wallet.map(), makeService: { throw BsvHomeBsvMultisigError.MissingRecord })
        })
    }

    private func option(_ payer: Primitives.Wallet) -> BsvHomeBsvMultisigFundingPayer {
        BsvHomeBsvMultisigFundingPayer(walletId: payer.id.id, name: payer.name, address: payer.accounts[0].address)
    }

    private func expectRetired(_ preparation: BsvHomeSendPublicFundingPreparation, fixture: SendPublicPaymailFixture) async {
        await #expect(throws: BsvHomePaymailError.Auth(error: .Service(error: .Cancelled))) {
            try await fixture.service.confirmPublicFunding(preparation: preparation, value: 42000)
        }
    }

    private enum CandidateFailure: Error {
        case unavailable
    }

    private actor CandidateGate {
        private var continuation: CheckedContinuation<Void, Never>?
        private var startedContinuation: CheckedContinuation<Void, Never>?
        private var hasStarted = false

        func wait() async {
            await withCheckedContinuation {
                continuation = $0
                hasStarted = true
                startedContinuation?.resume()
                startedContinuation = nil
            }
        }

        func started() async {
            if hasStarted {
                return
            }
            await withCheckedContinuation { startedContinuation = $0 }
        }

        func release() {
            continuation?.resume()
            continuation = nil
        }
    }
}
