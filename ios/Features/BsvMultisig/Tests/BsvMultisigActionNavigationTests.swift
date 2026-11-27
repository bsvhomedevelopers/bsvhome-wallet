@testable import BsvMultisig
import Components
import Foundation
import BsvHomeCore
import Observation
import SwiftUI
import Testing
import UIKit

@MainActor
@Suite(.serialized)
struct BsvMultisigActionNavigationTests {
    @Test(.timeLimit(.minutes(1)), arguments: [false, true])
    func internalPushAndRebuildPreserveTheActionUntilActualExit(background: Bool) async throws {
        let animations = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
        defer { UIView.setAnimationsEnabled(animations) }
        let service = try await BsvMultisigServiceMock.scoped()
        let gate = BsvMultisigAsyncGate()
        service.onAction = { _ in await gate.wait() }
        let refreshGate = BsvMultisigAsyncGate()
        service.onRefreshProposal = {
            await refreshGate.wait()
            return BsvMultisigFixtures.proposal()
        }
        let cancelled = Signal()
        var completions = 0
        let model = BsvMultisigViewModel(makeService: { service }, cancelAuthorization: { _ in cancelled.send() })
        await model.load()
        let state = HostState(model: model)
        state.owner.binding.wrappedValue.append(BsvMultisigRoute.review("proposal", purpose: .sign))
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.rootViewController = UIHostingController(rootView: Host(state: state))
        window.makeKeyAndVisible()
        defer {
            model.cancel()
            window.isHidden = true
            window.rootViewController = nil
        }
        await state.reviewAppeared.wait()
        await refreshGate.started()
        await state.refreshStarted.wait()
        await refreshGate.release()
        await state.refreshed.wait()
        service.onRefreshProposal = nil
        #expect(!model.isWorking)
        let task = try #require(model.sign(BsvMultisigFixtures.review(), navigation: state.owner) { _ in completions += 1 })
        await gate.started()
        let scope = try #require(service.actionScopes.last)
        state.owner.binding.wrappedValue.append(BsvMultisigRoute.settings("multisig"))
        await state.childAppeared.wait()
        #expect(throws: Never.self) { try scope.check() }
        state.revision += 1
        await state.rebuilt.wait()
        #expect(model.isWorking)
        state.owner.removeLast()
        await state.reviewAppeared.wait()
        #expect(throws: Never.self) { try scope.check() }
        if background {
            state.phase = .background
        } else {
            state.owner.reset()
            #expect(!model.isWorking)
        }
        await cancelled.wait()
        #expect(try scope.outcome().submission == .cancelledBeforeSubmission)
        await gate.release()
        #expect(await task.value == nil)
        #expect(completions == 0)
        #expect(!service.calls.contains("cancel"))
    }

    @Observable
    @MainActor
    final class HostState {
        let model: BsvMultisigViewModel
        let owner = NavigationPathState()
        var phase: ScenePhase = .active
        var revision = 0
        let reviewAppeared = Signal()
        let childAppeared = Signal()
        let rebuilt = Signal()
        let refreshStarted = Signal()
        let refreshed = Signal()
        init(model: BsvMultisigViewModel) {
            self.model = model
        }
    }

    private struct Host: View {
        @Bindable var state: HostState
        var body: some View {
            NavigationStack(path: state.owner.binding) {
                Color.clear
                    .navigationDestination(for: BsvMultisigRoute.self) { route in
                        switch route {
                        case .review:
                            BsvMultisigReviewScene(model: state.model, proposalId: "proposal", purpose: .sign)
                                .onAppear { state.reviewAppeared.send() }
                        default:
                            Text("Public navigation fixture")
                                .onAppear { state.childAppeared.send() }
                        }
                    }
            }
            .environment(state.owner)
            .environment(\.scenePhase, state.phase)
            .environment(\.locale, Locale(identifier: state.revision.isMultiple(of: 2) ? "en_US" : "zh_Hans"))
            .onChange(of: state.revision) { _, _ in state.rebuilt.send() }
            .onChange(of: state.model.isWorking) { _, isWorking in
                if isWorking {
                    state.refreshStarted.send()
                } else {
                    state.refreshed.send()
                }
            }
        }
    }

    @MainActor
    final class Signal {
        private let pair = AsyncStream<Void>.makeStream()
        func send() {
            pair.continuation.yield(())
        }

        func wait() async {
            var iterator = pair.stream.makeAsyncIterator()
            _ = await iterator.next()
        }
    }
}
