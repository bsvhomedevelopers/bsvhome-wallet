@testable import BsvColdWallet
import Components
import Foundation
import BsvHomeCore
import Observation
import SwiftUI
import Testing
import UIKit

@MainActor
@Suite(.serialized)
struct BsvColdWalletActionNavigationTests {
    @Test(.timeLimit(.minutes(1)), arguments: Exit.allCases)
    func aRealPathExitCancelsTheRunningScopeBeforeTheBindingAssignmentReturns(exit: Exit) async throws {
        let action = Action.sign
        let setup = try await setup(action)
        setup.model.path = []
        let state = HostState(model: setup.model)
        let window = host(state)
        defer {
            setup.model.cancel()
            window.isHidden = true
            window.rootViewController = nil
            Task { await setup.reply.complete(setup.result) }
        }
        await state.coldRootAppeared.wait()
        setup.model.open(action.page)
        await state.pageRendered.wait()
        try #require(state.owner.path.count == state.entry.count + 1)
        start(action, model: setup.model, navigation: state.owner)
        let task = try #require(setup.model.operationTask)
        await setup.reply.waitForEntry()
        let scope = try #require(setup.service.lastScope)
        #expect(throws: Never.self) { try scope.check() }
        let expectedPending = setup.model.pending
        var replacement = state.entry
        if exit == .replace {
            replacement.append(HostRoute.replacement)
        }
        state.owner.binding.wrappedValue = replacement
        #expect(throws: BsvHomeServiceError.Cancelled) { try scope.check() }
        #expect(setup.cancelled.values == [scope.id()])
        #expect(!setup.model.isBusy)
        #expect(state.owner.path == replacement)
        await setup.reply.complete(setup.result)
        await task.value
        #expect(setup.model.pending == expectedPending)
        #expect(state.owner.path == replacement)
    }

    @Test(.timeLimit(.minutes(1)))
    func internalPushRebuildAndBackPreserveTheRunningScopeUntilItsPageIsRemoved() async throws {
        let action = Action.sign
        let setup = try await setup(action)
        setup.model.path = []
        let state = HostState(model: setup.model)
        let window = host(state)
        defer {
            setup.model.cancel()
            window.isHidden = true
            window.rootViewController = nil
            Task { await setup.reply.complete(setup.result) }
        }
        await state.coldRootAppeared.wait()
        setup.model.open(action.page)
        await state.pageRendered.wait()
        try #require(state.owner.path.count == state.entry.count + 1)
        start(action, model: setup.model, navigation: state.owner)
        let task = try #require(setup.model.operationTask)
        await setup.reply.waitForEntry()
        let scope = try #require(setup.service.lastScope)
        let actionPath = state.owner.path
        state.owner.binding.wrappedValue.append(HostRoute.child)
        await state.childAppeared.wait()
        #expect(throws: Never.self) { try scope.check() }
        state.revision += 1
        await state.rebuilt.wait()
        #expect(throws: Never.self) { try scope.check() }
        #expect(setup.model.isBusy)
        state.owner.binding.wrappedValue = actionPath
        #expect(throws: Never.self) { try scope.check() }
        #expect(setup.cancelled.values.isEmpty)
        state.owner.removeLast()
        #expect(throws: BsvHomeServiceError.Cancelled) { try scope.check() }
        #expect(setup.cancelled.values == [scope.id()])
        await setup.reply.complete(setup.result)
        await task.value
    }

    @Test(arguments: Action.allCases)
    func exitingBeforeTheFirstTaskPollEntersNoServiceOperation(action: Action) async throws {
        let setup = try await setup(action)
        let calls = EntryCount()
        let result = setup.result
        setup.service.signWithScopeReply = nil
        setup.service.prepareReply = { await calls.enter(); return result }
        setup.service.signReply = { await calls.enter(); return result }
        setup.service.broadcastReply = { await calls.enter(); return result }
        let owner = NavigationPathState()
        owner.append(HostRoute.entry)
        owner.append(action.page)
        start(action, model: setup.model, navigation: owner)
        let task = try #require(setup.model.operationTask)
        let scope = try #require(setup.service.lastScope)
        owner.removeLast()
        #expect(setup.cancelled.values == [scope.id()])
        await task.value
        #expect(await calls.value == 0)
        #expect(!setup.model.isBusy)
    }

    @Test
    func removingAnOldOwnerCannotCancelTheReplacementAction() async throws {
        let setup = try await setup(.sign)
        let first = NavigationPathState()
        first.append(HostRoute.entry)
        first.append(BsvColdWalletRoute.review)
        start(.sign, model: setup.model, navigation: first)
        let oldTask = try #require(setup.model.operationTask)
        await setup.reply.waitForEntry()
        let oldScope = try #require(setup.service.lastScope)
        #expect(throws: Never.self) { try oldScope.check() }
        setup.model.cancelActions()
        await setup.reply.complete(setup.result)
        await oldTask.value
        await setup.model.load()
        setup.model.pending = setup.service.fixturePending
        setup.model.path = [.review]
        setup.model.isReviewConfirmed = true
        let second = NavigationPathState()
        second.append(HostRoute.entry)
        second.append(BsvColdWalletRoute.review)
        start(.sign, model: setup.model, navigation: second)
        let nextTask = try #require(setup.model.operationTask)
        defer { Task { await setup.reply.complete(setup.result) } }
        await setup.reply.waitForEntry()
        let nextScope = try #require(setup.service.lastScope)
        try #require(nextScope.id() != oldScope.id())
        #expect(throws: Never.self) { try nextScope.check() }
        first.removeLast()
        #expect(throws: Never.self) { try nextScope.check() }
        #expect(setup.cancelled.values == [oldScope.id()])
        second.removeLast()
        #expect(throws: BsvHomeServiceError.Cancelled) { try nextScope.check() }
        #expect(setup.cancelled.values == [oldScope.id(), nextScope.id()])
        await setup.reply.complete(setup.result)
        await nextTask.value
    }

    @Test
    func reconcileDoesNotAdoptOrOverwriteAForeignSameDepthReplacement() {
        let owner = NavigationPathState()
        owner.append(HostRoute.entry)
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .unified)
        let navigation = BsvColdWalletNavigation(navigation: owner)
        model.attachNavigationRetirement { navigation.retire() }
        model.path = [.review]
        navigation.show(model.path, model: model)
        var foreign = NavigationPath()
        foreign.append(HostRoute.entry)
        foreign.append(HostRoute.replacement)
        owner.binding.wrappedValue = foreign
        navigation.reconcile(model: model)
        model.path.append(.result)
        navigation.show(model.path, model: model)
        #expect(owner.path == foreign)
        model.cancel()
        #expect(owner.path == foreign)
    }

    @Test
    func anOwnedProjectionRebindsOnlyItsActionAndPreservesTheContextObserver() async throws {
        let setup = try await setup(.sign)
        let owner = NavigationPathState()
        owner.append(HostRoute.entry)
        var contextExits = 0
        owner.observeRemoval { contextExits += 1 }
        let navigation = BsvColdWalletNavigation(navigation: owner)
        navigation.show(setup.model.path, model: setup.model)
        start(.sign, model: setup.model, navigation: owner)
        let task = try #require(setup.model.operationTask)
        defer { Task { await setup.reply.complete(setup.result) } }
        await setup.reply.waitForEntry()
        let scope = try #require(setup.service.lastScope)
        #expect(throws: Never.self) { try scope.check() }
        setup.model.path = [.signed]
        navigation.show(setup.model.path, model: setup.model)
        #expect(throws: Never.self) { try scope.check() }
        #expect(setup.cancelled.values.isEmpty)
        #expect(contextExits == 0)
        owner.removeLast()
        #expect(throws: BsvHomeServiceError.Cancelled) { try scope.check() }
        #expect(contextExits == 0)
        owner.reset()
        #expect(contextExits == 1)
        await setup.reply.complete(setup.result)
        await task.value
    }

    @Test
    func anOverriddenOwnedProjectionDoesNotRebindTheOldActionToAForeignPath() async throws {
        let setup = try await setup(.sign)
        let owner = NavigationPathState()
        owner.append(HostRoute.entry)
        let navigation = BsvColdWalletNavigation(navigation: owner)
        navigation.show(setup.model.path, model: setup.model)
        start(.sign, model: setup.model, navigation: owner)
        let task = try #require(setup.model.operationTask)
        defer { Task { await setup.reply.complete(setup.result) } }
        await setup.reply.waitForEntry()
        let scope = try #require(setup.service.lastScope)
        var foreign = NavigationPath()
        foreign.append(HostRoute.entry)
        foreign.append(HostRoute.replacement)
        owner.observeRemoval { owner.binding.wrappedValue = foreign }
        setup.model.path = [.signed]
        navigation.show(setup.model.path, model: setup.model)
        #expect(owner.path == foreign)
        #expect(throws: BsvHomeServiceError.Cancelled) { try scope.check() }
        #expect(setup.cancelled.values == [scope.id()])
        navigation.show([.result], model: setup.model)
        #expect(owner.path == foreign)
        await setup.reply.complete(setup.result)
        await task.value
    }

    private func setup(_ action: Action) async throws -> Setup {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        let reply = BsvColdWalletAsyncReply<BsvHomeBsvColdWalletPending>()
        let wallet = try await service.installScopes(onSigning: { _, _, _, _ in
            _ = try await reply.wait()
            throw BsvHomeServiceError.Cancelled
        })
        var initial = action == .sign ? BsvColdWalletServiceMock.fixture : BsvColdWalletServiceMock.observerFixture
        initial.wallet = wallet
        initial.pending = action == .sign ? service.fixturePending : nil
        let snapshot = initial
        service.snapshotReply = { snapshot }
        service.candidatesReply = { [.init(wallet: wallet, role: snapshot.role)] }
        service.currentWalletIdReply = { wallet.id }
        let cancelled = Cancellations()
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: wallet.id, cancelAuthorization: { cancelled.values.append($0) })
        await model.load()
        try #require(!model.isBusy)
        try #require(!model.loadFailed)
        try #require(model.snapshot?.wallet == wallet)
        var pending = BsvColdWalletServiceMock.signed
        pending.review.walletId = wallet.id
        if action == .sign {
            pending.signedJson = nil; pending.transactionId = nil
        }
        model.pending = action == .sign ? service.fixturePending : (action == .prepare ? nil : pending)
        model.recipient = "1BitcoinEaterAddressDontSendf59kuE"
        model.amount = "0.000001"
        model.path = [action.page]
        model.isReviewConfirmed = true
        if action == .sign {
            service.signWithScopeReply = { [weak service] scope in
                guard let service else { throw BsvHomeServiceError.Cancelled }
                try await service.executeFixtureSign(scope: scope)
                throw BsvHomeServiceError.Cancelled
            }
        }
        service.prepareReply = { try await reply.wait() }
        service.signReply = { try await reply.wait() }
        service.broadcastReply = { try await reply.wait() }
        var completed = BsvColdWalletServiceMock.signed
        completed.review.walletId = wallet.id
        completed.broadcasted = action == .broadcast
        if action == .prepare {
            completed.signedJson = nil; completed.transactionId = nil
        }
        return Setup(service: service, model: model, reply: reply, result: completed, cancelled: cancelled)
    }

    private func start(_ action: Action, model: BsvColdWalletViewModel, navigation: NavigationPathState) {
        switch action {
        case .prepare: model.prepare(navigation: navigation)
        case .sign: model.sign(navigation: navigation)
        case .broadcast: model.broadcast(navigation: navigation)
        }
    }

    private func host(_ state: HostState) -> UIWindow {
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.rootViewController = UIHostingController(rootView: Host(state: state))
        window.makeKeyAndVisible()
        return window
    }

    enum Action: CaseIterable, Sendable { case prepare, sign, broadcast
        var page: BsvColdWalletRoute {
            switch self { case .prepare: .send; case .sign: .review; case .broadcast: .broadcast }
        }
    }

    enum Exit: CaseIterable, Sendable { case pop, replace }
    enum HostRoute: String, Hashable, Codable, Sendable { case entry, child, replacement }
    struct Setup {
        let service: BsvColdWalletServiceMock
        let model: BsvColdWalletViewModel
        let reply: BsvColdWalletAsyncReply<BsvHomeBsvColdWalletPending>
        let result: BsvHomeBsvColdWalletPending
        let cancelled: Cancellations
    }

    @MainActor final class Cancellations { var values: [String] = [] }
    actor EntryCount { private(set) var value = 0; func enter() {
        value += 1
    } }
    @Observable @MainActor final class HostState {
        let model: BsvColdWalletViewModel
        let owner: NavigationPathState
        let entry: NavigationPath
        var revision = 0
        var isInstalled = false
        var installedModel: BsvColdWalletViewModel? {
            guard isInstalled, owner.path.count >= entry.count else { return nil }
            var prefix = owner.path
            prefix.removeLast(prefix.count - entry.count)
            return prefix == entry ? model : nil
        }

        let coldRootAppeared = Signal()
        let pageRendered = Signal()
        let childAppeared = Signal()
        let rebuilt = Signal()
        init(model: BsvColdWalletViewModel) {
            let navigation = NavigationPathState()
            navigation.append(HostRoute.entry)
            owner = navigation
            entry = navigation.path
            self.model = model
        }
    }

    @MainActor final class Signal {
        private let pair = AsyncStream<Void>.makeStream()
        func send() {
            pair.continuation.yield(())
        }

        func wait() async {
            var iterator = pair.stream.makeAsyncIterator(); _ = await iterator.next()
        }
    }

    private struct Host: View {
        @Bindable var state: HostState
        var body: some View {
            NavigationStack(path: state.owner.binding) {
                Color.clear.navigationDestination(for: HostRoute.self) { route in
                    switch route {
                    case .entry:
                        BsvColdWalletScene(model: state.model, navigation: state.owner)
                            .onAppear { state.isInstalled = true; state.coldRootAppeared.send() }
                    case .child, .replacement:
                        Text("Public path fixture").onAppear { state.childAppeared.send() }
                    }
                }
                .bsvColdWalletNavigation(model: state.installedModel, navigation: state.owner)
            }
            .environment(state.owner)
            .environment(\.scenePhase, .active)
            .environment(\.locale, Locale(identifier: state.revision.isMultiple(of: 2) ? "en_US" : "zh_Hans"))
            .onChange(of: state.owner.path) { _, path in
                if path.count == state.entry.count + 1 {
                    state.pageRendered.send()
                }
            }
            .onChange(of: state.revision) { _, _ in state.rebuilt.send() }
        }
    }
}
