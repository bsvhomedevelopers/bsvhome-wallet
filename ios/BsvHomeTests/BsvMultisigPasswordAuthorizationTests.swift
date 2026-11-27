import BsvHomeCorePrimitivesTestKit
import Foundation
@testable import BsvHome
import BsvHomeCore
import BsvHomeCorePrimitives
import Observation
import Primitives
import Testing
@testable import Transfer

@MainActor
struct BsvMultisigPasswordAuthorizationTests {
    @Test(.timeLimit(.minutes(1)))
    func authorizesTheActualPayerAndCompletedSheetDismissalDoesNotCancelItsScope() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        let payer = payer()
        let service = BsvHomePortalTransferPasswordServiceMock(snapshot: BsvHomeTransferPasswordSnapshot(walletId: payer.id, address: payer.accounts[0].address, configured: true, generation: 0))
        var requestedWallets: [Primitives.Wallet] = []
        let authorization = BsvMultisigPasswordAuthorization(makeService: { wallet in
            requestedWallets.append(wallet)
            return service
        }, isForeground: { true })
        let presentations = observePresentation(authorization)
        let task = Task { try await authorization.authorize(wallet: payer, request: fundingRequest(payer), scope: scope) }
        var iterator = presentations.makeAsyncIterator()
        let prompt = try #require(await iterator.next())
        let requestId = try #require(authorization.requestId)
        #expect(requestedWallets.isEmpty)
        #expect(prompt.walletName == payer.name)
        await prompt.load()
        #expect(requestedWallets == [payer.map()])
        prompt.currentPassword = "dummy-current"
        prompt.submit()
        try await (#require(prompt.submission)).value
        try await task.value
        authorization.dismissPassword(requestId: requestId)
        try scope.check()
        #expect(authorization.presentation == nil)
        #expect(service.verifyCount == 1)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test(.timeLimit(.minutes(1)))
    func anOldScopeOrSheetCallbackCannotCancelTheReplacementPrompt() async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let firstScope = try fixture.beginRetryFunding()
        let nextScope = try fixture.beginRetryFunding()
        let payer = payer()
        let authorization = BsvMultisigPasswordAuthorization(makeService: { _ in BsvHomePortalTransferPasswordServiceMock() }, isForeground: { true })
        let firstPresentations = observePresentation(authorization)
        let first = Task { try await authorization.authorize(wallet: payer, request: fundingRequest(payer), scope: firstScope) }
        var firstIterator = firstPresentations.makeAsyncIterator()
        _ = try #require(await firstIterator.next())
        let oldRequestId = try #require(authorization.requestId)
        authorization.cancel(scopeId: firstScope.id())
        await expectCancelled(first)

        let nextPresentations = observePresentation(authorization)
        let next = Task { try await authorization.authorize(wallet: payer, request: fundingRequest(payer), scope: nextScope) }
        var nextIterator = nextPresentations.makeAsyncIterator()
        let prompt = try #require(await nextIterator.next())
        prompt.currentPassword = "dummy-current"
        authorization.dismissPassword(requestId: oldRequestId)
        authorization.cancel(scopeId: firstScope.id())
        #expect(authorization.presentation?.id == prompt.id)
        try nextScope.check()
        authorization.cancel(scopeId: nextScope.id())
        #expect(authorization.presentation == nil)
        #expect(prompt.currentPassword.isEmpty)
        await expectCancelled(next)
        #expect(throws: BsvHomeBsvMultisigError.Cancelled) { try nextScope.check() }
    }

    @Test(arguments: [false, true])
    func inactiveOrAlreadyCancelledAdmissionDoesNotCreateAPasswordService(cancelled: Bool) async throws {
        let fixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try fixture.beginRetryFunding()
        if cancelled {
            scope.cancel()
        }
        var creations = 0
        let authorization = BsvMultisigPasswordAuthorization(makeService: { _ in
            creations += 1
            return BsvHomePortalTransferPasswordServiceMock()
        }, isForeground: { cancelled })
        let payer = payer()
        let task = Task { try await authorization.authorize(wallet: payer, request: fundingRequest(payer), scope: scope) }
        await expectCancelled(task)
        #expect(authorization.presentation == nil)
        #expect(creations == 0)
        #expect(fixture.networkRequestCount == 0)
        #expect(fixture.credentialAccessCount == 0)
    }

    private func payer() -> BsvHomeCore.Wallet {
        let address = "1BitcoinEaterAddressDontSendf59kuE"
        return BsvHomeCore.Wallet(id: Primitives.WalletId.privateKey(chain: .bitcoinSv, address: address).id, externalId: nil, name: "Public payer fixture", index: 0, walletType: .privateKey, accounts: [BsvHomeCore.Account(chain: Primitives.Chain.bitcoinSv.rawValue, address: address, derivationPath: "", extendedPublicKey: nil)], isPinned: false, imageUrl: nil, source: .import)
    }

    private func fundingRequest(_ wallet: BsvHomeCore.Wallet) -> BsvHomeBsvMultisigAuthorizationRequest {
        .funding(targetName: "Public target", commitment: "fixture-commitment", configurationHash: "fixture-configuration", senderAddress: wallet.accounts[0].address, amountSatoshis: "10000", feeSatoshis: "100", transactionId: nil)
    }

    private func observePresentation(_ authorization: BsvMultisigPasswordAuthorization) -> AsyncStream<TransferPasswordViewModel> {
        AsyncStream { continuation in
            withObservationTracking {
                _ = authorization.presentation
            } onChange: {
                Task { @MainActor in
                    if let presentation = authorization.presentation {
                        continuation.yield(presentation)
                    }
                    continuation.finish()
                }
            }
        }
    }

    private func expectCancelled(_ task: Task<Void, any Error>) async {
        do {
            try await task.value
            Issue.record("Expected scoped authorization cancellation")
        } catch {
            #expect(error as? BsvHomeServiceError == .Cancelled)
        }
    }
}
