// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeTransferData
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Observation
import Primitives
import PrimitivesComponents
import PrimitivesTestKit
import SwiftUI
import Testing
@testable import Transfer
import TransferTestKit
import UIKit

@MainActor
@Suite(.serialized)
struct ConfirmExecutionNavigationTests {
    @Test(.timeLimit(.minutes(1)), arguments: [false, true])
    func internalNavigationPreservesAuthorizationUntilExit(background: Bool) async throws {
        let animationsEnabled = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
        defer { UIView.setAnimationsEnabled(animationsEnabled) }

        let asset = Asset.mock(id: .mock(.bitcoinSv), name: "Bitcoin SV", symbol: "BSV")
        let data = BsvHomeTransferData.mock(type: .transfer(asset: asset.map()))
        let wallet = Primitives.Wallet.mock(accounts: [.mock(chain: .bitcoinSv)])
        let passwordService = BsvHomePortalTransferPasswordServiceMock()
        let authorization = TransferPasswordAuthorization(makeService: { _ in passwordService })
        let session = BsvHomeConfirmSessionMock()
        let cancelled = AsyncStream<Void>.makeStream()
        session.onCancel = {
            cancelled.continuation.yield()
        }
        let started = Signal()
        var completed = false
        session.onExecuteAsync = {
            started.send()
            try await authorization.authorize(wallet: wallet.map(), transfer: data)
            return .signed(data: ["authorized"])
        }
        let model = ConfirmTransferSceneViewModel(
            request: .mock(data: data),
            wallet: wallet,
            session: session,
            passwordAuthorization: authorization,
            onComplete: { completed = true },
        )
        let state = HostState()
        state.presentation.present(model: model)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.rootViewController = UIHostingController(rootView: Host(state: state))
        window.makeKeyAndVisible()
        defer {
            model.cancelExecution()
            window.isHidden = true
            window.rootViewController = nil
        }
        await state.rootAppeared.wait()
        await model.load()
        state.path = [.fee]
        await state.feeAppeared.wait()
        #expect(session.cancelCount == 0)

        state.revision += 1
        await state.rebuilt.wait()
        #expect(session.cancelCount == 0)

        state.path = []
        await state.rootAppeared.wait()
        await model.load()
        model.onSelectConfirm()
        let execution = try #require(model.executionTask)
        await started.wait()
        let presentation = try #require(authorization.presentation)
        state.revision += 1
        await state.rebuilt.wait()
        #expect(session.cancelCount == 0)
        #expect(authorization.presentation === presentation)
        #expect(model.isConfirming)

        if background {
            state.phase = .background
        } else {
            state.presentation.isPresented = false
            try #require(session.cancelCount > 0)
            #expect(state.presentation.model == nil)
        }
        var cancellations = cancelled.stream.makeAsyncIterator()
        _ = await cancellations.next()
        await execution.value

        #expect(session.cancelCount > 0)
        #expect(authorization.presentation == nil)
        #expect(model.executionTask == nil)
        #expect(!model.isConfirming)
        #expect(!completed)
    }

    enum Route: Hashable {
        case fee
    }

    @Observable
    @MainActor
    final class HostState {
        var path: [Route] = []
        var phase: ScenePhase = .active
        var revision = 0
        let presentation = ConfirmTransferPresentation()
        let rootAppeared = Signal()
        let feeAppeared = Signal()
        let rebuilt = Signal()
    }

    private struct Host: View {
        @Bindable var state: HostState

        var body: some View {
            Group {
                if state.presentation.isPresented, let model = state.presentation.model {
                    NavigationStack(path: $state.path) {
                        ConfirmTransferScene(model: model)
                            .id(model.id)
                            .onAppear { state.rootAppeared.send() }
                            .navigationDestination(for: Route.self) { _ in
                                NetworkFeeScene(model: model.feeModel)
                                    .onAppear { state.feeAppeared.send() }
                            }
                    }
                    .environment(\.scenePhase, state.phase)
                    .environment(\.locale, Locale(identifier: state.revision.isMultiple(of: 2) ? "en_US" : "zh_Hans"))
                } else {
                    Color.clear
                }
            }
            .onChange(of: state.revision) { _, _ in state.rebuilt.send() }
        }
    }

    @MainActor
    final class Signal {
        private let pair = AsyncStream<Void>.makeStream()

        func send() {
            pair.continuation.yield()
        }

        func wait() async {
            var iterator = pair.stream.makeAsyncIterator()
            _ = await iterator.next()
        }
    }
}
