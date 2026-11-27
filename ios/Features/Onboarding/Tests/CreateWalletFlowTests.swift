// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeWalletService
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
@testable import Onboarding
import Testing

@MainActor
struct CreateWalletFlowTests {
    @Test
    func generatedPhraseSurvivesNavigationAndIsClearedOnExit() throws {
        let model = CreateWalletModel(
            service: BsvHomeWalletService.mock(),
            preferences: .mock(),
            avatarService: BsvHomeAvatarServiceMock(),
            onComplete: nil,
        )
        var steps: [CreateWalletFlow.Step] = []
        let flow = CreateWalletFlow(model: model) { steps.append($0) }

        flow.revealSecretPhrase()
        let words = try #require(flow.generatedWords)
        #expect(steps == [.showSecret])
        #expect(words.count == 12)

        flow.revealSecretPhrase()
        #expect(flow.generatedWords == words)

        flow.verifySecretPhrase()
        #expect(steps.last == .verifyPhrase)
        #expect(flow.generatedWords == words)

        flow.discard()
        #expect(flow.generatedWords == nil)
        #expect(flow.createdWallet == nil)
    }

    @Test(arguments: [false, true])
    func dedicatedCreationRetriesTheOriginalImportResult(existing: Bool) async throws {
        let service = MultisigIdentityWalletServiceMock(noHandle: .init())
        let original = service.result
        if existing, case let .new(wallet) = original {
            service.result = .existing(wallet: wallet)
        }
        var completedWalletIds: [String] = []
        var personalCompletions = 0
        let model = CreateWalletModel(
            service: service,
            preferences: .mock(),
            avatarService: BsvHomeAvatarServiceMock(),
            onComplete: { personalCompletions += 1 },
            purpose: .dedicatedMultisigIdentity { completedWalletIds.append($0.id.id) },
        )
        var steps: [CreateWalletFlow.Step] = []
        let flow = CreateWalletFlow(model: model) { steps.append($0) }
        flow.revealSecretPhrase()
        let words = try #require(flow.generatedWords)

        do {
            try await flow.completeVerification(words: words)
            Issue.record("Expected metadata completion to fail")
        } catch {
            #expect(completedWalletIds.isEmpty)
        }
        #expect(service.importCount == 1)
        #expect(flow.createdWallet?.importResult == service.result)
        #expect(flow.generatedWords != nil)
        let candidate = CreateWalletFlow(model: model) { _ in Issue.record("An uninstalled flow must not navigate") }
        #expect(candidate.createdWallet?.importResult == service.result)
        #expect(candidate.generatedWords != nil)

        service.completionError = nil
        try await flow.completeVerification(words: words)

        #expect(service.importCount == 1)
        #expect(service.completionResults == [service.result, service.result])
        #expect(service.completionSources == [.create, .create])
        #expect(completedWalletIds.count == 1)
        #expect(personalCompletions == 0)
        #expect(service.selectedWalletIds.isEmpty)
        #expect(!steps.contains(.walletProfile))
        #expect(flow.createdWallet == nil)
        #expect(flow.generatedWords == nil)
    }

    @Test
    func cancellingBeforeCreationStartsDoesNotImportOrCompleteIdentity() async throws {
        let service = MultisigIdentityWalletServiceMock(noHandle: .init())
        var completions = 0
        let model = CreateWalletModel(
            service: service,
            preferences: .mock(),
            avatarService: BsvHomeAvatarServiceMock(),
            onComplete: nil,
            purpose: .dedicatedMultisigIdentity { _ in completions += 1 },
        )
        try model.revealSecretPhrase()
        let creation = try model.startCreation(words: #require(model.generatedWords))
        model.cancel()
        do {
            _ = try await creation.value
            Issue.record("Expected cancelled creation")
        } catch {
            #expect(error is CancellationError)
        }

        #expect(creation.isCancelled)
        #expect(service.importCount == 0)
        #expect(service.completionResults.isEmpty)
        #expect(service.selectedWalletIds.isEmpty)
        #expect(completions == 0)
        #expect(model.generatedWords == nil)
        #expect(model.createdWallet == nil)
        #expect(model.creationTask == nil)
    }

    @Test(arguments: [false, true])
    func cancelledCreationCannotAcceptALateImport(dedicated: Bool) async throws {
        let service = MultisigIdentityWalletServiceMock(noHandle: .init())
        service.completionError = nil
        let pending = AsyncStream<CheckedContinuation<Void, Never>>.makeStream()
        service.onImport = { await withCheckedContinuation { pending.continuation.yield($0) } }
        var completions = 0
        let model = CreateWalletModel(
            service: service,
            preferences: .mock(),
            avatarService: BsvHomeAvatarServiceMock(),
            onComplete: { completions += 1 },
            purpose: dedicated ? .dedicatedMultisigIdentity { _ in completions += 1 } : .personal,
        )
        var steps: [CreateWalletFlow.Step] = []
        let flow = CreateWalletFlow(model: model) { steps.append($0) }
        flow.revealSecretPhrase()
        let words = try #require(flow.generatedWords)
        let creation = Task { try await flow.completeVerification(words: words) }
        var requests = pending.stream.makeAsyncIterator()
        let completion = try #require(await requests.next())
        let ownedTask = try #require(model.creationTask)
        model.cancel()
        completion.resume()
        do {
            try await creation.value
            Issue.record("Expected cancelled creation")
        } catch {
            #expect(error is CancellationError)
        }

        #expect(ownedTask.isCancelled)
        #expect(service.importCount == 1)
        #expect(service.completionResults.isEmpty)
        #expect(service.selectedWalletIds.isEmpty)
        #expect(completions == 0)
        #expect(!steps.contains(.walletProfile))
        #expect(flow.generatedWords == nil)
        #expect(flow.createdWallet == nil)
        #expect(model.creationTask == nil)
    }
}
