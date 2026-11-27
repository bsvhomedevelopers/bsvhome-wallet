import BsvHomeCorePrimitivesTestKit
// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeTransferPasswordError
import struct BsvHomeCore.BsvHomeTransferPasswordSnapshot
import BsvHomeCoreServicesTestKit
import Localization
import Testing
@testable import Transfer

@MainActor
struct TransferPasswordViewModelTests {
    @Test
    func configuredAuthorizationVerifiesAndClearsTheInputBeforeAwaiting() async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        var matchedInput = false
        var completions = 0
        service.onVerify = { _, password in matchedInput = password == "dummy-current" }
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Fixture wallet", makeService: { service }, onComplete: { completions += 1 })
        await model.load()
        model.currentPassword = "dummy-current"

        model.submit()
        let submission = try #require(model.submission)
        #expect(model.currentPassword.isEmpty)
        await submission.value

        #expect(matchedInput)
        #expect(service.verifyCount == 1)
        #expect(service.setCount == 0)
        #expect(completions == 1)
        #expect(model.isComplete)
        model.submit()
        #expect(service.verifyCount == 1)
    }

    @Test(arguments: [TransferPasswordMode.authorization, .management])
    func initialSetupOmitsTheCurrentPasswordAndCompletesWithoutVerifying(mode: TransferPasswordMode) async throws {
        let service = BsvHomePortalTransferPasswordServiceMock(snapshot: snapshot(configured: false))
        let configuredSnapshot = snapshot()
        var matchedInput = false
        var completions = 0
        service.onSet = { _, current, new, confirmation in
            matchedInput = current == nil && new == "dummy-new" && confirmation == "dummy-new"
            return configuredSnapshot
        }
        let model = TransferPasswordViewModel(mode: mode, walletName: "Fixture wallet", makeService: { service }, onComplete: { completions += 1 })
        await model.load()
        #expect(!model.showsCurrentPassword)
        model.newPassword = "dummy-new"
        model.confirmation = "dummy-new"
        model.submit()
        let submission = try #require(model.submission)
        #expect(model.newPassword.isEmpty && model.confirmation.isEmpty)
        await submission.value

        #expect(matchedInput)
        #expect(service.setCount == 1)
        #expect(service.verifyCount == 0)
        #expect(completions == 1)
    }

    @Test
    func managementRequiresAndForwardsTheCurrentPassword() async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        var matchedInput = false
        service.onSet = { snapshot, current, new, confirmation in
            matchedInput = current == "dummy-current" && new == "dummy-new" && confirmation == "dummy-new"
            return snapshot
        }
        let model = TransferPasswordViewModel(mode: .management, walletName: "Fixture wallet", makeService: { service })
        await model.load()
        model.newPassword = "dummy-new"
        model.confirmation = "dummy-new"
        #expect(!model.canSubmit)
        model.currentPassword = "dummy-current"
        model.submit()
        try await (#require(model.submission)).value

        #expect(matchedInput)
        #expect(service.setCount == 1)
        #expect(service.verifyCount == 0)
        #expect(model.isComplete)
    }

    @Test
    func repeatedAppearSharesTheOwnedStatusRequest() async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        let pending = AsyncStream<CheckedContinuation<BsvHomeTransferPasswordSnapshot, any Error>>.makeStream()
        service.onStatus = { try await withCheckedThrowingContinuation { pending.continuation.yield($0) } }
        var serviceCreations = 0
        let model = TransferPasswordViewModel(mode: .management, walletName: "Fixture wallet", makeService: {
            serviceCreations += 1
            return service
        })

        model.startLoading()
        let loading = try #require(model.loadingTask)
        model.startLoading()
        var requests = pending.stream.makeAsyncIterator()
        let completion = try #require(await requests.next())
        model.startLoading()
        model.reloadStatus()
        #expect(service.statusCount == 1)
        #expect(serviceCreations == 1)
        #expect(!loading.isCancelled)

        let expected = snapshot()
        completion.resume(returning: expected)
        await loading.value
        model.startLoading()

        #expect(model.state.value == expected)
        #expect(model.loadingTask == nil)
        #expect(service.statusCount == 1)
        #expect(serviceCreations == 1)
        #expect(!model.isCancelled)
    }

    @Test
    func appearingAfterStatusFailureRequiresExplicitRetry() async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        service.onStatus = { throw BsvHomeTransferPasswordError.Unavailable }
        let model = TransferPasswordViewModel(mode: .management, walletName: "Fixture wallet", makeService: { service })
        model.startLoading()
        try await (#require(model.loadingTask)).value
        #expect(model.state.isError)
        #expect(model.loadingTask == nil)

        model.startLoading()
        #expect(model.state.isError)
        #expect(model.loadingTask == nil)
        #expect(service.statusCount == 1)

        let expected = snapshot(generation: 1)
        service.onStatus = { expected }
        model.reloadStatus()
        try await (#require(model.loadingTask)).value

        #expect(model.state.value == expected)
        #expect(service.statusCount == 2)
        #expect(!model.isCancelled)
    }

    @Test(arguments: [TransferPasswordMode.authorization, .management])
    func cancelledStatusEndsTheActionWithoutOfferingAnUnusableRetry(mode: TransferPasswordMode) async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        service.onStatus = { throw BsvHomeTransferPasswordError.Cancelled }
        var cancellations = 0
        let model = TransferPasswordViewModel(mode: mode, walletName: "Fixture wallet", makeService: { service }, onCancel: { cancellations += 1 })
        model.startLoading()
        try await (#require(model.loadingTask)).value
        model.startLoading()
        model.reloadStatus()
        await model.load()

        #expect(cancellations == 1)
        #expect(service.cancelCount == 1)
        #expect(service.statusCount == 1)
        #expect(!model.canSubmit)
        #expect(!model.isComplete)
        #expect(model.isCancelled)
        #expect(model.outcome == .cancelled)
        #expect(model.loadingTask == nil)
    }

    @Test
    func cancellingBeforeOwnedLoadingStartsDoesNotCreateAService() async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        var serviceCreations = 0
        let model = TransferPasswordViewModel(mode: .management, walletName: "Fixture wallet", makeService: {
            serviceCreations += 1
            return service
        })
        model.startLoading()
        let loading = try #require(model.loadingTask)
        model.cancel()
        await loading.value

        #expect(loading.isCancelled)
        #expect(serviceCreations == 0)
        #expect(service.statusCount == 0)
        #expect(model.loadingTask == nil)
        #expect(model.isCancelled)
        #expect(model.state.value == nil)
    }

    @Test
    func lateStatusCannotReviveACancelledAction() async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        let pending = AsyncStream<CheckedContinuation<BsvHomeTransferPasswordSnapshot, any Error>>.makeStream()
        service.onStatus = { try await withCheckedThrowingContinuation { pending.continuation.yield($0) } }
        var completions = 0
        var cancellations = 0
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Fixture wallet", makeService: { service }, onComplete: { completions += 1 }, onCancel: { cancellations += 1 })
        model.startLoading()
        let loading = try #require(model.loadingTask)
        var requests = pending.stream.makeAsyncIterator()
        let completion = try #require(await requests.next())
        model.currentPassword = "dummy-current"
        model.cancel()
        model.cancel()
        #expect(loading.isCancelled)
        #expect(model.loadingTask == nil)
        completion.resume(returning: snapshot())
        await loading.value
        model.startLoading()
        model.reloadStatus()

        #expect(completions == 0)
        #expect(cancellations == 1)
        #expect(service.cancelCount == 1)
        #expect(service.statusCount == 1)
        #expect(model.currentPassword.isEmpty)
        #expect(model.state.value == nil)
        #expect(model.loadingTask == nil)
        #expect(model.isCancelled)
        #expect(model.outcome == .cancelled)
    }

    @Test(arguments: [false, true])
    func cancellingBeforeSubmissionStartsDoesNotSetOrVerify(setting: Bool) async throws {
        let service = BsvHomePortalTransferPasswordServiceMock(snapshot: snapshot(configured: !setting))
        var completions = 0
        var cancellations = 0
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Fixture wallet", makeService: { service }, onComplete: { completions += 1 }, onCancel: { cancellations += 1 })
        await model.load()
        model.currentPassword = "dummy-current"
        model.newPassword = "dummy-new"
        model.confirmation = "dummy-new"
        model.submit()
        let submission = try #require(model.submission)
        model.cancel()
        await submission.value

        #expect(submission.isCancelled)
        #expect(service.setCount == 0)
        #expect(service.verifyCount == 0)
        #expect(service.cancelCount == 1)
        #expect(cancellations == 1)
        #expect(completions == 0)
        #expect(model.currentPassword.isEmpty && model.newPassword.isEmpty && model.confirmation.isEmpty)
        #expect(model.isCancelled)
        #expect(!model.isComplete)
        #expect(!model.isWorking)
        #expect(model.submission == nil)
    }

    @Test(arguments: [false, true])
    func lateSetAndVerifyCannotCompleteAfterCancellation(setting: Bool) async throws {
        let service = BsvHomePortalTransferPasswordServiceMock(snapshot: snapshot(configured: !setting))
        let pending = AsyncStream<CheckedContinuation<Void, any Error>>.makeStream()
        let pause: @MainActor () async throws -> Void = {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        service.onSet = { snapshot, _, _, _ in try await pause(); return snapshot }
        service.onVerify = { _, _ in try await pause() }
        var completions = 0
        var cancellations = 0
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Fixture wallet", makeService: { service }, onComplete: { completions += 1 }, onCancel: { cancellations += 1 })
        await model.load()
        model.currentPassword = "dummy-current"
        model.newPassword = "dummy-new"
        model.confirmation = "dummy-new"
        model.submit()
        let submission = try #require(model.submission)
        var requests = pending.stream.makeAsyncIterator()
        let completion = try #require(await requests.next())
        model.cancel()
        completion.resume()
        await submission.value

        #expect(completions == 0)
        #expect(cancellations == 1)
        #expect(service.cancelCount == 1)
        #expect(!model.isComplete)
        #expect(model.currentPassword.isEmpty && model.newPassword.isEmpty && model.confirmation.isEmpty)
    }

    @Test(arguments: [false, true])
    func typedSetAndVerifyCancellationCancelsOnce(setting: Bool) async throws {
        let service = BsvHomePortalTransferPasswordServiceMock(snapshot: snapshot(configured: !setting))
        service.onVerify = { _, _ in throw BsvHomeTransferPasswordError.Cancelled }
        service.onSet = { _, _, _, _ in throw BsvHomeTransferPasswordError.Cancelled }
        var completions = 0
        var cancellations = 0
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Fixture wallet", makeService: { service }, onComplete: { completions += 1 }, onCancel: { cancellations += 1 })
        await model.load()
        model.currentPassword = "dummy-current"
        model.newPassword = "dummy-new"
        model.confirmation = "dummy-new"
        model.submit()
        try await (#require(model.submission)).value

        #expect(completions == 0)
        #expect(cancellations == 1)
        #expect(service.cancelCount == 1)
        #expect(!model.canSubmit)
    }

    @Test(arguments: [BsvHomeTransferPasswordError.IncorrectPassword, .RateLimited])
    func aRejectedPasswordCanBeRetriedWithTheSameStatus(error: BsvHomeTransferPasswordError) async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Fixture wallet", makeService: { service })
        await model.load()
        service.onVerify = { _, _ in throw error }
        model.currentPassword = "dummy-current"
        model.submit()
        try await (#require(model.submission)).value
        #expect(model.state.value != nil)
        #expect(model.errorMessage == error.localizedDescription)
        #expect(!model.canSubmit)

        service.onVerify = nil
        model.currentPassword = "dummy-current"
        model.submit()
        try await (#require(model.submission)).value
        #expect(service.statusCount == 1)
        #expect(service.verifyCount == 2)
        #expect(model.isComplete)
    }

    @Test(arguments: [BsvHomeTransferPasswordError.Unavailable, .InvalidResponse, .InvalidSnapshot, .NotConfigured])
    func invalidatedStatusMustReloadBeforeAnotherSubmission(error: BsvHomeTransferPasswordError) async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Fixture wallet", makeService: { service })
        await model.load()
        service.onVerify = { _, _ in throw error }
        model.currentPassword = "dummy-current"
        model.submit()
        try await (#require(model.submission)).value
        #expect(model.state.isError)
        model.currentPassword = "dummy-current"
        model.submit()
        #expect(service.verifyCount == 1)

        let refreshed = snapshot(generation: 1)
        var usedRefreshedStatus = false
        service.onStatus = { refreshed }
        service.onVerify = { snapshot, _ in usedRefreshedStatus = snapshot == refreshed }
        await model.load()
        model.currentPassword = "dummy-current"
        model.submit()
        try await (#require(model.submission)).value

        #expect(usedRefreshedStatus)
        #expect(service.statusCount == 2)
        #expect(service.verifyCount == 2)
        #expect(model.isComplete)
    }

    @Test(arguments: [false, true])
    func unknownErrorsNeverExposeTheirMessage(submitting: Bool) async throws {
        let service = BsvHomePortalTransferPasswordServiceMock()
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Fixture wallet", makeService: { service })
        let unknown = NSError(domain: "sensitive-fixture", code: 7, userInfo: [NSLocalizedDescriptionKey: "sensitive-backend-message"])
        if submitting {
            await model.load()
            service.onVerify = { _, _ in throw unknown }
            model.currentPassword = "dummy-current"
            model.submit()
            try await (#require(model.submission)).value
        } else {
            service.onStatus = { throw unknown }
            await model.load()
        }
        guard case let .error(error) = model.state else { Issue.record("Expected a sanitized status error"); return }
        #expect(error.localizedDescription == Localized.wowpayTransferPasswordUnavailable)
        #expect(model.errorMessage == nil)
        #expect(!model.isComplete)
    }

    private func snapshot(configured: Bool = true, generation: UInt64 = 0) -> BsvHomeTransferPasswordSnapshot {
        BsvHomeTransferPasswordSnapshot(walletId: "test-wallet", address: "1BitcoinEaterAddressDontSendf59kuE", configured: configured, generation: generation)
    }
}
