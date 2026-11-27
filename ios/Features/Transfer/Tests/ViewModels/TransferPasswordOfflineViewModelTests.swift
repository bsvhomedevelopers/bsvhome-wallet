// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeTransferPasswordError
import struct BsvHomeCore.BsvHomeTransferPasswordSnapshot
import BsvHomeCorePrimitivesTestKit
import Localization
import Testing
@testable import Transfer

@MainActor
struct TransferPasswordOfflineViewModelTests {
    @Test(arguments: [false, true])
    func localStatusAndPasswordWorkRunInAnOwnedDetachedTask(setting: Bool) async throws {
        let calls = AsyncStream<(onMainThread: Bool, inheritedOwner: Bool)>.makeStream()
        let expected = snapshot(configured: !setting)
        let record: @Sendable () -> Void = {
            calls.continuation.yield((Thread.isMainThread, TransferPasswordOfflineTaskContext.owner != nil))
        }
        let service = BsvHomeOfflineSigningPasswordServiceMock(
            snapshot: expected,
            onStatus: { record(); return expected },
            onSet: { snapshot, _, _, _ in record(); return snapshot },
            onVerify: { _, _ in record() },
        )
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Local signing wallet", backend: .offline { service })
        try await TransferPasswordOfflineTaskContext.$owner.withValue(UUID()) {
            await model.load()
            fill(model)
            model.submit()
            let task = try #require(model.submission)
            #expect(model.currentPassword.isEmpty && model.newPassword.isEmpty && model.confirmation.isEmpty)
            await task.value
        }
        #expect(model.isComplete)
        #expect(service.statusCount == 1)
        #expect(service.setCount == (setting ? 1 : 0))
        #expect(service.verifyCount == (setting ? 0 : 1))
        var entries = calls.stream.makeAsyncIterator()
        for _ in 0 ..< 2 {
            let entry = try #require(await entries.next())
            #expect(!entry.onMainThread)
            #expect(!entry.inheritedOwner)
        }
    }

    @Test
    func cancellationBeforeFirstStatusPollNeverConstructsTheLocalService() async throws {
        let service = BsvHomeOfflineSigningPasswordServiceMock()
        var creations = 0
        let model = TransferPasswordViewModel(mode: .management, walletName: "Local wallet", backend: .offline {
            creations += 1
            return service
        })
        model.startLoading()
        let loading = try #require(model.loadingTask)
        model.cancel()
        await loading.value
        #expect(creations == 0)
        #expect(service.statusCount == 0)
        #expect(service.cancelCount == 0)
        #expect(model.isCancelled)
    }

    @Test(arguments: [false, true])
    func cancellationBeforeFirstSubmissionPollDoesNotEnterLocalPasswordWork(setting: Bool) async throws {
        let service = BsvHomeOfflineSigningPasswordServiceMock(snapshot: snapshot(configured: !setting))
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Local wallet", backend: .offline { service })
        await model.load()
        fill(model)
        model.submit()
        let submission = try #require(model.submission)
        model.cancel()
        await submission.value
        #expect(service.setCount == 0 && service.verifyCount == 0)
        #expect(service.cancelCount >= 1)
        #expect(model.isCancelled && !model.isComplete)
    }

    @Test(.timeLimit(.minutes(1)))
    func cancelInvalidatesCoreBeforeCancellingTheOwnedWorkerAndIgnoresLateCompletion() async throws {
        let pending = AsyncStream<CheckedContinuation<Void, any Error>>.makeStream()
        let service = BsvHomeOfflineSigningPasswordServiceMock(onVerify: { _, _ in
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        })
        var completions = 0
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Local wallet", backend: .offline { service }, onComplete: { completions += 1 })
        await model.load()
        fill(model)
        model.submit()
        let submission = try #require(model.submission)
        service.observeCancellation(of: submission)
        var entries = pending.stream.makeAsyncIterator()
        let completion = try #require(await entries.next())
        model.cancel()
        #expect(service.taskCancellationAtCancel.first == false)
        completion.resume()
        await submission.value
        #expect(completions == 0)
        #expect(model.isCancelled && !model.isComplete)
        #expect(model.currentPassword.isEmpty && model.newPassword.isEmpty && model.confirmation.isEmpty)
    }

    @Test(arguments: [BsvHomeTransferPasswordError.IncorrectPassword, .RateLimited, .CurrentPasswordRequired, .NotConfigured, .InvalidSnapshot])
    func localTypedFailuresUseLocalContextAndKeepCoreRetryPolicy(error: BsvHomeTransferPasswordError) async throws {
        let service = BsvHomeOfflineSigningPasswordServiceMock(onVerify: { _, _ in throw error })
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Local wallet", backend: .offline { service })
        await model.load()
        fill(model)
        model.submit()
        try await (#require(model.submission)).value
        #expect(!model.isComplete)
        switch error {
        case .IncorrectPassword, .CurrentPasswordRequired:
            #expect(model.errorMessage == Localized.wowpayOfflineSigningPasswordIncorrect)
        case .RateLimited:
            #expect(model.errorMessage == Localized.wowpayOfflineSigningPasswordRateLimited)
        case .NotConfigured:
            #expect(model.state.isError)
            #expect(model.errorDescription(error) == Localized.wowpayOfflineSigningPasswordUnavailable)
        case .InvalidSnapshot:
            #expect(model.state.isError)
            #expect(model.errorDescription(error) == Localized.wowpayTransferPasswordWalletChanged)
        default:
            Issue.record("Unexpected typed fixture case")
        }
        #expect(service.verifyCount == 1)
    }

    @Test(arguments: [false, true])
    func localUnknownFailuresNeverExposeBackendMessages(submitting: Bool) async throws {
        let unknown = NSError(domain: "local-fixture", code: 1, userInfo: [NSLocalizedDescriptionKey: "fixture-sensitive-message"])
        let expected = snapshot()
        let service = BsvHomeOfflineSigningPasswordServiceMock(
            onStatus: {
                if !submitting {
                    throw unknown
                }
                return expected
            },
            onVerify: { _, _ in throw unknown },
        )
        let model = TransferPasswordViewModel(mode: .authorization, walletName: "Local wallet", backend: .offline { service })
        await model.load()
        if submitting {
            fill(model)
            model.submit()
            try await (#require(model.submission)).value
        }
        guard case let .error(error) = model.state else { Issue.record("Expected a sanitized local status error"); return }
        #expect(model.errorDescription(error) == Localized.wowpayOfflineSigningPasswordUnavailable)
        #expect(model.errorMessage == nil)
        #expect(!model.isComplete)
    }

    private func fill(_ model: TransferPasswordViewModel) {
        model.currentPassword = "dummy-local-current"
        model.newPassword = "dummy-local-new"
        model.confirmation = "dummy-local-new"
    }

    private nonisolated func snapshot(configured: Bool = true) -> BsvHomeTransferPasswordSnapshot {
        BsvHomeTransferPasswordSnapshot(walletId: "test-wallet", address: "1BitcoinEaterAddressDontSendf59kuE", configured: configured, generation: 0)
    }
}
