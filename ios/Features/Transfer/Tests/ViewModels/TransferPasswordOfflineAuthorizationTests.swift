// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeOfflineSigningPasswordServiceProtocol
import enum BsvHomeCore.BsvHomeCoreError
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import Observation
import Primitives
import PrimitivesTestKit
import Testing
@testable import Transfer

@MainActor
struct TransferPasswordOfflineAuthorizationTests {
    @Test(.timeLimit(.minutes(1)))
    func explicitLocalAuthorizationDefersItsServiceAndResumesOnlyItsOwnPrompt() async throws {
        let service = BsvHomeOfflineSigningPasswordServiceMock()
        let authorization = TransferPasswordAuthorization.offline()
        var creations = 0
        let presentations = observePresentation(authorization)
        let requestId = UUID()
        let request = start(authorization, requestId: requestId, makeService: { creations += 1; return service })
        var iterator = presentations.makeAsyncIterator()
        let prompt = try #require(await iterator.next())
        #expect(creations == 0)
        #expect(authorization.requestId == requestId)
        await prompt.load()
        prompt.currentPassword = "dummy-local-current"
        prompt.submit()
        try await (#require(prompt.submission)).value
        try await request.value
        #expect(creations == 1)
        #expect(service.verifyCount == 1)
        #expect(prompt.isComplete)
        #expect(authorization.presentation == nil)
        #expect(authorization.requestId == nil)
    }

    @Test
    func localRequestCancelledBeforeFirstPollDoesNotCheckConstructOrPresent() async {
        let authorization = TransferPasswordAuthorization.offline()
        var checks = 0
        var creations = 0
        let service = BsvHomeOfflineSigningPasswordServiceMock()
        let request = start(authorization, check: { checks += 1 }, makeService: { creations += 1; return service })
        request.cancel()
        await expectCancelled(request)
        #expect(checks == 0)
        #expect(creations == 0)
        #expect(service.statusCount == 0)
        #expect(authorization.presentation == nil)
    }

    @Test(.timeLimit(.minutes(1)))
    func invalidationBeforeLocalPromptLoadingDoesNotConstructAService() async throws {
        let authorization = TransferPasswordAuthorization.offline()
        var allowed = true
        var creations = 0
        let presentations = observePresentation(authorization)
        let request = start(authorization, check: { guard allowed else { throw BsvHomeCoreError.Cancelled } }, makeService: {
            creations += 1
            return BsvHomeOfflineSigningPasswordServiceMock()
        })
        var iterator = presentations.makeAsyncIterator()
        let prompt = try #require(await iterator.next())
        allowed = false
        await prompt.load()
        await expectCancelled(request)
        #expect(creations == 0)
        #expect(prompt.isCancelled)
        #expect(authorization.presentation == nil)
    }

    @Test(.timeLimit(.minutes(1)))
    func lateLocalCompletionAndOldRequestCancellationCannotCloseANewerPrompt() async throws {
        let pending = AsyncStream<CheckedContinuation<Void, any Error>>.makeStream()
        let oldService = BsvHomeOfflineSigningPasswordServiceMock(onVerify: { _, _ in
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        })
        let authorization = TransferPasswordAuthorization.offline()
        let oldId = UUID()
        let oldPresentations = observePresentation(authorization)
        let oldRequest = start(authorization, requestId: oldId, makeService: { oldService })
        var oldIterator = oldPresentations.makeAsyncIterator()
        let oldPrompt = try #require(await oldIterator.next())
        await oldPrompt.load()
        oldPrompt.currentPassword = "dummy-local-current"
        oldPrompt.submit()
        let oldSubmission = try #require(oldPrompt.submission)
        var work = pending.stream.makeAsyncIterator()
        let oldCompletion = try #require(await work.next())
        authorization.cancel(requestId: oldId)
        await expectCancelled(oldRequest)

        let newId = UUID()
        let nextService = BsvHomeOfflineSigningPasswordServiceMock()
        let nextPresentations = observePresentation(authorization)
        let nextRequest = start(authorization, requestId: newId, makeService: { nextService })
        var nextIterator = nextPresentations.makeAsyncIterator()
        let nextPrompt = try #require(await nextIterator.next())
        oldCompletion.resume()
        await oldSubmission.value
        authorization.cancel(requestId: oldId)
        oldPrompt.cancel()
        #expect(oldPrompt.isCancelled)
        #expect(authorization.requestId == newId)
        #expect(authorization.presentation?.id == nextPrompt.id)
        await nextPrompt.load()
        nextPrompt.currentPassword = "dummy-local-current"
        nextPrompt.submit()
        try await (#require(nextPrompt.submission)).value
        try await nextRequest.value
        #expect(nextService.verifyCount == 1)
        #expect(authorization.presentation == nil)
    }

    @Test
    func localOnlyAuthorizationCannotEnterImplicitOrExplicitOnlineConfirmation() async {
        let authorization = TransferPasswordAuthorization.offline()
        let wallet = Wallet.mock(name: "Local wallet", accounts: [.mock(chain: .bitcoinSv, address: "1BitcoinEaterAddressDontSendf59kuE")])
        let implicit = Task { try await authorization.authorize(wallet: wallet.map(), transfer: .mock(type: .transfer(Chain.bitcoinSv.asset))) }
        await expectCancelled(implicit)
        var creations = 0
        let explicit = Task {
            try await authorization.authorize(wallet: wallet.map(), requestId: UUID(), check: {}, makeService: {
                creations += 1
                return BsvHomePortalTransferPasswordServiceMock()
            })
        }
        await expectCancelled(explicit)
        #expect(creations == 0)
        #expect(authorization.presentation == nil)
    }

    private func start(
        _ authorization: TransferPasswordAuthorization,
        requestId: UUID = UUID(),
        check: @escaping @MainActor () throws -> Void = {},
        makeService: @escaping @MainActor () throws -> any BsvHomeOfflineSigningPasswordServiceProtocol,
    ) -> Task<Void, any Error> {
        let wallet = Wallet.mock(name: "Local wallet", accounts: [.mock(chain: .bitcoinSv, address: "1BitcoinEaterAddressDontSendf59kuE")])
        return Task { try await authorization.authorizeOffline(wallet: wallet.map(), requestId: requestId, check: check, makeService: makeService) }
    }

    private func observePresentation(_ authorization: TransferPasswordAuthorization) -> AsyncStream<TransferPasswordViewModel> {
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

    private func expectCancelled(_ request: Task<Void, any Error>) async {
        do {
            try await request.value
            Issue.record("Expected local authorization cancellation")
        } catch {
            #expect(error as? BsvHomeCoreError == .Cancelled)
        }
    }
}
