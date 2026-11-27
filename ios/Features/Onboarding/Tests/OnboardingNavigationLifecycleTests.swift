// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeAppStartFailure
import protocol BsvHomeCore.BsvHomeAppStartServiceProtocol
import enum BsvHomeCore.BsvHomeServiceError
import class BsvHomeCore.BsvHomeWalletService
import struct BsvHomeCore.Wallet
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Observation
@testable import Onboarding
import Primitives
import SwiftUI
import Testing
import UIKit

@MainActor
@Suite(.serialized)
struct OnboardingNavigationLifecycleTests {
    @Test(.timeLimit(.minutes(1)))
    func flowPagesPreserveThePublicFixtureUntilExplicitCancellation() async {
        let service = MultisigIdentityWalletServiceMock(noHandle: .init())
        let model = createModel(service: service)
        let state = CreateHostState(model: model)
        let window = host(CreatePagesHost(state: state))
        defer { close(window, cancel: model.cancel) }
        await state.securityAppeared.wait()

        state.flow.revealSecretPhrase()
        await state.secretAppeared.wait()
        #expect(model.generatedWords?.count == 12)
        #expect(service.importCount == 0)
        state.flow.verifySecretPhrase()
        await state.verifyAppeared.wait()
        #expect(model.generatedWords?.count == 12)

        state.revision += 1
        await state.rebuilt.wait()
        #expect(model.generatedWords?.count == 12)
        #expect(model.createdWallet == nil)
        #expect(service.importCount == 0)
        state.path.removeLast()
        await state.secretAppeared.wait()
        #expect(model.generatedWords?.count == 12)

        model.cancel()
        await state.cleared.wait()
        state.isPresented = false
        await state.removed.wait()
        #expect(model.generatedWords == nil)
        #expect(model.creationTask == nil)
        #expect(model.createdWallet == nil)
        #expect(service.completionResults.isEmpty)
    }

    @Test(.timeLimit(.minutes(1)))
    func cancellationOnAPushedFlowPageRejectsALateImport() async throws {
        let gate = Gate()
        let service = MultisigIdentityWalletServiceMock(noHandle: .init())
        service.completionError = nil
        service.onImport = { await gate.wait() }
        var completions = 0
        let model = createModel(service: service, onComplete: { completions += 1 })
        let state = CreateHostState(model: model)
        let window = host(CreatePagesHost(state: state))
        defer {
            gate.open()
            close(window, cancel: model.cancel)
        }
        await state.securityAppeared.wait()
        state.flow.revealSecretPhrase()
        await state.secretAppeared.wait()
        state.flow.verifySecretPhrase()
        await state.verifyAppeared.wait()
        let creation = Task { try await state.flow.completeVerification(words: LocalKeystore.words) }
        await gate.started.wait()
        let ownedTask = try #require(model.creationTask)

        state.revision += 1
        await state.rebuilt.wait()
        #expect(!ownedTask.isCancelled)
        #expect(model.generatedWords?.count == 12)
        model.cancel()
        state.isPresented = false
        #expect(ownedTask.isCancelled)
        #expect(model.creationTask == nil)
        await state.removed.wait()
        gate.open()
        do {
            try await creation.value
            Issue.record("A cancelled creation must not complete")
        } catch {
            #expect(error is CancellationError)
        }

        #expect(completions == 0)
        #expect(service.importCount == 1)
        #expect(service.completionResults.isEmpty)
        #expect(service.selectedWalletIds.isEmpty)
        #expect(model.generatedWords == nil)
        #expect(model.createdWallet == nil)
    }

    @Test(.timeLimit(.minutes(1)))
    func actualCreateStackCancelsOwnedCreationOnRootBackground() async throws {
        let gate = Gate()
        let service = MultisigIdentityWalletServiceMock(noHandle: .init())
        service.completionError = nil
        service.onImport = { await gate.wait() }
        var completions = 0
        let model = createModel(service: service, onComplete: { completions += 1 })
        let state = CreateHostState(model: model)
        let window = host(CreateStackHost(state: state))
        defer {
            gate.open()
            close(window, cancel: model.cancel)
        }
        await state.securityAppeared.wait()
        try model.revealSecretPhrase()
        let creation = try model.startCreation(words: LocalKeystore.words)
        await gate.started.wait()
        state.phase = .background
        await state.cleared.wait()
        #expect(creation.isCancelled)
        #expect(model.creationTask == nil)
        #expect(model.generatedWords == nil)
        gate.open()
        do {
            _ = try await creation.value
            Issue.record("The production stack must cancel creation when backgrounded")
        } catch {
            #expect(error is CancellationError)
        }
        #expect(service.completionResults.isEmpty)
        #expect(service.selectedWalletIds.isEmpty)
        #expect(completions == 0)
    }

    @Test(.timeLimit(.minutes(1)), arguments: [false, true])
    func pushedImportReviewCancelsLateAuthentication(background: Bool) async throws {
        let keystore = LocalKeystore.mock()
        let service = BsvHomeWalletService.mock(keystore: keystore)
        let gate = Gate()
        var completions = 0
        let parent = ImportWalletViewModel(service: service, keystore: keystore.bsvHomeKeystore, authenticate: { await gate.wait() }, appStartService: NoWalletSetup(), preferences: .mock(), nameService: BsvHomeNameServiceMock(), avatarService: BsvHomeAvatarServiceMock(), onComplete: nil)
        let model = parent.importWalletModel(type: .chain(.bitcoinSv), onComplete: { _ in completions += 1 })
        let state = ImportHostState(parent: parent, model: model)
        let window = host(ImportHost(state: state))
        defer {
            gate.open()
            close(window, cancel: parent.cancel)
        }
        await state.appeared.wait()
        model.input = LocalKeystore.words.joined(separator: " ")
        await model.onSelectActionButton()
        await state.reviewPushed.wait()
        #expect(model.preview != nil)
        #expect(!model.input.isEmpty)
        #expect(model.isPresentingHdReview)

        state.revision += 1
        await state.rebuilt.wait()
        #expect(model.preview != nil)
        #expect(!model.input.isEmpty)
        model.isHdConfirmed = true
        let authentication = Task { await model.confirmHdImport() }
        await gate.started.wait()
        if background {
            state.phase = .background
            await state.cleared.wait()
        } else {
            parent.cancel()
            #expect(model.preview == nil)
            #expect(model.input.isEmpty)
            state.isPresented = false
            await state.removed.wait()
        }
        gate.open()
        await authentication.value

        #expect(model.input.isEmpty)
        #expect(model.passphrase.isEmpty)
        #expect(model.preview == nil)
        #expect(!model.isHdConfirmed)
        #expect(!model.isBusy)
        #expect(completions == 0)
        #expect(try await service.getWallets().isEmpty)
        #expect(try service.currentWalletId() == nil)
    }

    private func createModel(service: MultisigIdentityWalletServiceMock, onComplete: @escaping @MainActor @Sendable () -> Void = {}) -> CreateWalletModel {
        let preferences = ObservablePreferences.mock()
        preferences.acceptTerms()
        return CreateWalletModel(service: service, preferences: preferences, avatarService: BsvHomeAvatarServiceMock(), onComplete: nil, purpose: .dedicatedMultisigIdentity { _ in onComplete() })
    }

    private func host(_ content: some View) -> UIWindow {
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.rootViewController = UIHostingController(rootView: content)
        window.makeKeyAndVisible()
        return window
    }

    private func close(_ window: UIWindow, cancel: () -> Void) {
        cancel()
        window.isHidden = true
        window.rootViewController = nil
    }

    @Observable
    @MainActor
    fileprivate final class CreateHostState {
        let model: CreateWalletModel
        @ObservationIgnored private(set) var flow: CreateWalletFlow!
        var path: [CreateWalletFlow.Step] = []
        var phase = ScenePhase.active
        var revision = 0
        var isPresented = true
        let securityAppeared = Signal()
        let secretAppeared = Signal()
        let verifyAppeared = Signal()
        let rebuilt = Signal()
        let cleared = Signal()
        let removed = Signal()

        init(model: CreateWalletModel) {
            self.model = model
            flow = CreateWalletFlow(model: model) { [weak self] step in self?.path.append(step) }
        }
    }

    private struct CreatePagesHost: View {
        @Bindable var state: CreateHostState

        var body: some View {
            Group {
                if state.isPresented {
                    NavigationStack(path: $state.path) {
                        CreateWalletFlowScene(flow: state.flow, step: .securityReminder)
                            .onAppear { state.securityAppeared.send() }
                            .navigationDestination(for: CreateWalletFlow.Step.self) { step in
                                CreateWalletFlowScene(flow: state.flow, step: step)
                                    .onAppear {
                                        switch step {
                                        case .showSecret: state.secretAppeared.send()
                                        case .verifyPhrase: state.verifyAppeared.send()
                                        default: break
                                        }
                                    }
                            }
                    }
                    .environment(\.scenePhase, state.phase)
                    .environment(\.locale, Locale(identifier: state.revision.isMultiple(of: 2) ? "en_US" : "zh_Hans"))
                } else {
                    Color.clear.onAppear { state.removed.send() }
                }
            }
            .onChange(of: state.revision) { _, _ in state.rebuilt.send() }
            .onChange(of: state.model.generatedWords?.count) {
                _, current in if current == nil {
                    state.cleared.send()
                }
            }
        }
    }

    private struct CreateStackHost: View {
        @Bindable var state: CreateHostState

        var body: some View {
            CreateWalletNavigationStack(model: state.model, entry: .securityReminder)
                .environment(\.scenePhase, state.phase)
                .onAppear { state.securityAppeared.send() }
                .onChange(of: state.model.generatedWords?.count) {
                    _, current in if current == nil {
                        state.cleared.send()
                    }
                }
        }
    }

    @Observable
    @MainActor
    fileprivate final class ImportHostState {
        let parent: ImportWalletViewModel
        let model: ImportWalletSceneViewModel
        var phase = ScenePhase.active
        var revision = 0
        var isPresented = true
        let appeared = Signal()
        let reviewPushed = Signal()
        let rebuilt = Signal()
        let cleared = Signal()
        let removed = Signal()

        init(parent: ImportWalletViewModel, model: ImportWalletSceneViewModel) {
            self.parent = parent
            self.model = model
        }
    }

    private struct ImportHost: View {
        @Bindable var state: ImportHostState

        var body: some View {
            Group {
                if state.isPresented {
                    NavigationStack {
                        ImportWalletScene(model: state.model)
                            .onAppear {
                                state.parent.registerInputModel(state.model)
                                state.appeared.send()
                            }
                            .onDisappear {
                                if state.model.isPresentingHdReview {
                                    state.reviewPushed.send()
                                }
                            }
                    }
                    .environment(\.scenePhase, state.phase)
                    .environment(\.locale, Locale(identifier: state.revision.isMultiple(of: 2) ? "en_US" : "zh_Hans"))
                } else {
                    Color.clear.onAppear { state.removed.send() }
                }
            }
            .onChange(of: state.revision) { _, _ in state.rebuilt.send() }
            .onChange(of: state.model.input.isEmpty) {
                _, empty in if empty {
                    state.cleared.send()
                }
            }
        }
    }

    @MainActor
    fileprivate final class Signal {
        private let events = AsyncStream<Void>.makeStream()

        func send() {
            events.continuation.yield()
        }

        func wait() async {
            var iterator = events.stream.makeAsyncIterator()
            _ = await iterator.next()
        }
    }

    @MainActor
    private final class Gate {
        let started = Signal()
        private var continuation: CheckedContinuation<Void, Never>?

        func wait() async {
            await withCheckedContinuation { continuation in
                self.continuation = continuation
                started.send()
            }
        }

        func open() {
            continuation?.resume()
            continuation = nil
        }
    }

    private final class NoWalletSetup: BsvHomeAppStartServiceProtocol, Sendable {
        func initializeWalletAssets(wallet _: BsvHomeCore.Wallet) async throws {
            throw BsvHomeServiceError.InvalidInput(msg: "Wallet setup is outside this lifecycle check")
        }

        func run() async -> [BsvHomeAppStartFailure] {
            []
        }

        func setupWallet(wallet _: BsvHomeCore.Wallet) async -> [BsvHomeAppStartFailure] {
            []
        }

        func setupWallets() async -> [BsvHomeAppStartFailure] {
            []
        }
    }
}
