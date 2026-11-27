// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomePortalTransferPasswordServiceProtocol
import enum BsvHomeCore.BsvHomeCoreError
import struct BsvHomeCore.BsvHomeTransferPasswordSnapshot
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Observation
import Primitives
import PrimitivesTestKit
import Testing
@testable import Transfer

@MainActor
struct TransferPasswordAuthorizationTests {
    @Test
    func successfulVerificationResumesAuthorizationAndDismissesThePrompt() async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        let authorization = TransferPasswordAuthorization(makeService: { _ in service })
        let presentations = observePresentation(authorization)
        let request = startAuthorization(authorization)
        var iterator = presentations.makeAsyncIterator()
        let prompt = try #require(await iterator.next())
        await prompt.load()
        prompt.currentPassword = "dummy-current"
        prompt.submit()
        try await (#require(prompt.submission)).value
        try await request.value

        #expect(service.verifyCount == 1)
        #expect(authorization.presentation == nil)
        #expect(prompt.currentPassword.isEmpty)
        authorization.cancel()
        #expect(service.cancelCount == 0)
    }

    @Test(arguments: [false, true])
    func cancellingPendingAuthorizationResumesOnceAndDiscardsLateStatus(cancelTask: Bool) async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        let pending = AsyncStream<CheckedContinuation<BsvHomeTransferPasswordSnapshot, any Error>>.makeStream()
        service.onStatus = { try await withCheckedThrowingContinuation { pending.continuation.yield($0) } }
        let authorization = TransferPasswordAuthorization(makeService: { _ in service })
        let presentations = observePresentation(authorization)
        let request = startAuthorization(authorization)
        var iterator = presentations.makeAsyncIterator()
        let prompt = try #require(await iterator.next())
        let loading = Task { await prompt.load() }
        var statuses = pending.stream.makeAsyncIterator()
        let status = try #require(await statuses.next())
        prompt.currentPassword = "dummy-current"
        if cancelTask {
            request.cancel()
        } else {
            authorization.cancel()
        }
        await expectCancelled(request)
        authorization.cancel()
        status.resume(returning: BsvHomeTransferPasswordSnapshot(walletId: "test-wallet", address: "1BitcoinEaterAddressDontSendf59kuE", configured: true, generation: 0))
        await loading.value

        #expect(authorization.presentation == nil)
        #expect(service.cancelCount == 1)
        #expect(service.verifyCount == 0)
        #expect(prompt.currentPassword.isEmpty)
        #expect(prompt.state.value == nil)
    }

    @Test(arguments: [false, true])
    func aConcurrentRequestCannotReplaceTheActiveContinuation(walletOnly: Bool) async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        let authorization = TransferPasswordAuthorization(makeService: { _ in service })
        let presentations = observePresentation(authorization)
        let first = startAuthorization(authorization)
        var iterator = presentations.makeAsyncIterator()
        let prompt = try #require(await iterator.next())
        let second = walletOnly ? startWalletAuthorization(authorization, makeService: { service }) : startAuthorization(authorization)
        await expectCancelled(second)

        #expect(authorization.presentation?.id == prompt.id)
        authorization.cancel()
        await expectCancelled(first)
        #expect(authorization.presentation == nil)
    }

    @Test
    func walletAuthorizationUsesThePromptFactoryAndExternalRequestIdentity() async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        var ordinaryServiceCreations = 0
        var promptServiceCreations = 0
        let authorization = TransferPasswordAuthorization(makeService: { _ in
            ordinaryServiceCreations += 1
            return BsvHomePortalTransferPasswordServiceMock()
        })
        let requestId = UUID()
        let presentations = observePresentation(authorization)
        let request = startWalletAuthorization(authorization, requestId: requestId, makeService: {
            promptServiceCreations += 1
            return service
        })
        var iterator = presentations.makeAsyncIterator()
        let prompt = try #require(await iterator.next())
        #expect(authorization.requestId == requestId)
        #expect(prompt.walletName == "Scoped payer")
        #expect(promptServiceCreations == 0)
        await prompt.load()
        prompt.currentPassword = "dummy-current"
        prompt.submit()
        try await (#require(prompt.submission)).value
        try await request.value

        #expect(ordinaryServiceCreations == 0)
        #expect(promptServiceCreations == 1)
        #expect(service.verifyCount == 1)
        #expect(authorization.presentation == nil)
        #expect(authorization.requestId == nil)
    }

    @Test
    func walletAuthorizationCancelledBeforeFirstPollDoesNotCheckOrCreateAService() async {
        var checks = 0
        var serviceCreations = 0
        let authorization = TransferPasswordAuthorization(makeService: { _ in BsvHomePortalTransferPasswordServiceMock() })
        let request = startWalletAuthorization(authorization, check: { checks += 1 }, makeService: {
            serviceCreations += 1
            return BsvHomePortalTransferPasswordServiceMock()
        })
        request.cancel()
        await expectCancelled(request)

        #expect(checks == 0)
        #expect(serviceCreations == 0)
        #expect(authorization.presentation == nil)
        #expect(authorization.requestId == nil)
    }

    @Test
    func rejectedWalletAdmissionDoesNotPresentOrCreateAService() async {
        var serviceCreations = 0
        let authorization = TransferPasswordAuthorization(makeService: { _ in BsvHomePortalTransferPasswordServiceMock() })
        let request = startWalletAuthorization(authorization, check: { throw BsvHomeCoreError.Cancelled }, makeService: {
            serviceCreations += 1
            return BsvHomePortalTransferPasswordServiceMock()
        })
        await expectCancelled(request)

        #expect(serviceCreations == 0)
        #expect(authorization.presentation == nil)
        #expect(authorization.requestId == nil)
    }

    @Test
    func invalidationBeforePromptLoadingDoesNotConstructItsService() async throws {
        var isAllowed = true
        var serviceCreations = 0
        let authorization = TransferPasswordAuthorization(makeService: { _ in BsvHomePortalTransferPasswordServiceMock() })
        let presentations = observePresentation(authorization)
        let request = startWalletAuthorization(authorization, check: {
            guard isAllowed else { throw BsvHomeCoreError.Cancelled }
        }, makeService: {
            serviceCreations += 1
            return BsvHomePortalTransferPasswordServiceMock()
        })
        var iterator = presentations.makeAsyncIterator()
        let prompt = try #require(await iterator.next())
        isAllowed = false
        await prompt.load()
        await expectCancelled(request)

        #expect(serviceCreations == 0)
        #expect(prompt.isCancelled)
        #expect(authorization.presentation == nil)
    }

    @Test
    func invalidationDuringVerificationCannotCompleteWalletAuthorization() async throws {
        var isAllowed = true
        let service = BsvHomePortalTransferPasswordServiceMock()
        service.onVerify = { _, _ in isAllowed = false }
        let authorization = TransferPasswordAuthorization(makeService: { _ in BsvHomePortalTransferPasswordServiceMock() })
        let presentations = observePresentation(authorization)
        let request = startWalletAuthorization(authorization, check: {
            guard isAllowed else { throw BsvHomeCoreError.Cancelled }
        }, makeService: { service })
        var iterator = presentations.makeAsyncIterator()
        let prompt = try #require(await iterator.next())
        await prompt.load()
        prompt.currentPassword = "dummy-current"
        prompt.submit()
        try await (#require(prompt.submission)).value
        await expectCancelled(request)

        #expect(service.verifyCount == 1)
        #expect(prompt.currentPassword.isEmpty)
        #expect(authorization.presentation == nil)
        #expect(authorization.requestId == nil)
    }

    @Test
    func scopedCancellationClearsEveryInputSynchronouslyAndResumesOnce() async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        let authorization = TransferPasswordAuthorization(makeService: { _ in BsvHomePortalTransferPasswordServiceMock() })
        let requestId = UUID()
        let presentations = observePresentation(authorization)
        let request = startWalletAuthorization(authorization, requestId: requestId, makeService: { service })
        var iterator = presentations.makeAsyncIterator()
        let prompt = try #require(await iterator.next())
        await prompt.load()
        prompt.currentPassword = "dummy-current"
        prompt.newPassword = "dummy-new"
        prompt.confirmation = "dummy-new"

        authorization.cancel(requestId: requestId)
        #expect(authorization.presentation == nil)
        #expect(authorization.requestId == nil)
        #expect(prompt.currentPassword.isEmpty && prompt.newPassword.isEmpty && prompt.confirmation.isEmpty)
        #expect(prompt.isCancelled)
        #expect(service.cancelCount == 1)
        authorization.cancel(requestId: requestId)
        prompt.cancel()
        await expectCancelled(request)
        #expect(service.cancelCount == 1)
        #expect(service.verifyCount == 0 && service.setCount == 0)
    }

    @Test
    func anOldExternalRequestCannotCancelItsReplacement() async throws {
        let authorization = TransferPasswordAuthorization(makeService: { _ in BsvHomePortalTransferPasswordServiceMock() })
        let firstId = UUID()
        let firstPresentations = observePresentation(authorization)
        let first = startWalletAuthorization(authorization, requestId: firstId, makeService: { BsvHomePortalTransferPasswordServiceMock() })
        var firstIterator = firstPresentations.makeAsyncIterator()
        let oldPrompt = try #require(await firstIterator.next())
        authorization.cancel(requestId: firstId)
        await expectCancelled(first)

        let nextId = UUID()
        let nextPresentations = observePresentation(authorization)
        let next = startWalletAuthorization(authorization, requestId: nextId, makeService: { BsvHomePortalTransferPasswordServiceMock() })
        var nextIterator = nextPresentations.makeAsyncIterator()
        let nextPrompt = try #require(await nextIterator.next())
        authorization.cancel(requestId: firstId)
        oldPrompt.cancel()
        #expect(authorization.requestId == nextId)
        #expect(authorization.presentation?.id == nextPrompt.id)
        await nextPrompt.load()
        nextPrompt.currentPassword = "dummy-current"
        nextPrompt.submit()
        try await (#require(nextPrompt.submission)).value
        try await next.value
        #expect(authorization.presentation == nil)
    }

    @Test
    func aRequestCancelledBeforeItsFirstPollDoesNotPresentOrCreateAService() async {
        var createdService = false
        let authorization = TransferPasswordAuthorization(makeService: { _ in
            createdService = true
            return BsvHomePortalTransferPasswordServiceMock()
        })
        let request = startAuthorization(authorization)
        request.cancel()
        await expectCancelled(request)

        #expect(!createdService)
        #expect(authorization.presentation == nil)
    }

    @Test(arguments: [false, true])
    func lateCancellationFromAnOldRequestCannotCloseTheNextPrompt(completedFirst: Bool) async throws {
        let authorization = TransferPasswordAuthorization(makeService: { _ in BsvHomePortalTransferPasswordServiceMock() })
        let firstPresentations = observePresentation(authorization)
        let first = startAuthorization(authorization)
        var firstIterator = firstPresentations.makeAsyncIterator()
        let oldPrompt = try #require(await firstIterator.next())
        let oldRequestId = try #require(authorization.requestId)
        if completedFirst {
            await oldPrompt.load()
            oldPrompt.currentPassword = "dummy-current"
            oldPrompt.submit()
            try await (#require(oldPrompt.submission)).value
            try await first.value
        } else {
            authorization.cancel()
            await expectCancelled(first)
        }

        let secondPresentations = observePresentation(authorization)
        let second = startAuthorization(authorization)
        var secondIterator = secondPresentations.makeAsyncIterator()
        let newPrompt = try #require(await secondIterator.next())
        #expect(authorization.requestId != oldRequestId)
        authorization.cancel(requestId: oldRequestId)
        oldPrompt.cancel()
        #expect(authorization.presentation?.id == newPrompt.id)
        await newPrompt.load()
        newPrompt.currentPassword = "dummy-current"
        newPrompt.submit()
        try await (#require(newPrompt.submission)).value
        try await second.value
        #expect(newPrompt.isComplete)
        #expect(authorization.presentation == nil)
    }

    private func startAuthorization(_ authorization: TransferPasswordAuthorization) -> Task<Void, any Error> {
        let wallet = Wallet.mock(name: "Fixture wallet", accounts: [.mock(chain: .bitcoinSv, address: "1BitcoinEaterAddressDontSendf59kuE")])
        return Task { try await authorization.authorize(wallet: wallet.map(), transfer: .mock(type: .transfer(Chain.bitcoinSv.asset))) }
    }

    private func startWalletAuthorization(
        _ authorization: TransferPasswordAuthorization,
        requestId: UUID = UUID(),
        check: @escaping @MainActor () throws -> Void = {},
        makeService: @escaping @MainActor () throws -> any BsvHomePortalTransferPasswordServiceProtocol,
    ) -> Task<Void, any Error> {
        let wallet = Wallet.mock(name: "Scoped payer", accounts: [.mock(chain: .bitcoinSv, address: "1BitcoinEaterAddressDontSendf59kuE")])
        return Task { try await authorization.authorize(wallet: wallet.map(), requestId: requestId, check: check, makeService: makeService) }
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
            Issue.record("Expected authorization cancellation")
        } catch {
            #expect(error as? BsvHomeCoreError == .Cancelled)
        }
    }
}
