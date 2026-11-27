@testable import BsvColdWallet
import Components
import SwiftUI
import Testing

@MainActor
struct BsvColdWalletRootNavigationTests {
    @Test
    func rebuildingTheSceneSharesTheSameWorkflowAndDoesNotRebindItsActualOwner() {
        let model = makeModel()
        let owner = NavigationPathState()
        let otherOwner = NavigationPathState()
        let navigation = model.navigation(in: owner)
        #expect(model.navigation(in: owner) === navigation)
        #expect(model.navigation(in: otherOwner) === navigation)
        model.path = [.signing]
        navigation.show(model.path, model: model)
        let route = navigation.pageRoute(.signing, at: 0)
        #expect(navigation.owns(route, model: model, navigation: owner))
        #expect(!navigation.owns(route, model: model, navigation: otherOwner))
        #expect(otherOwner.path.isEmpty)
    }

    @Test
    func theStableRootRendersOnlyTheMatchingWorkflowPageAndIndex() {
        let model = makeModel()
        let owner = NavigationPathState()
        owner.append("cold-entry")
        let navigation = model.navigation(in: owner)
        model.path = [.signing, .review]
        navigation.show(model.path, model: model)
        let signing = navigation.pageRoute(.signing, at: 0)
        let review = navigation.pageRoute(.review, at: 1)
        #expect(navigation.owns(signing, model: model, navigation: owner))
        #expect(navigation.owns(review, model: model, navigation: owner))
        #expect(!navigation.owns(navigation.pageRoute(.review, at: 0), model: model, navigation: owner))
        #expect(!navigation.owns(navigation.pageRoute(.review, at: 2), model: model, navigation: owner))
        let replacement = makeModel()
        replacement.path = model.path
        let replacementNavigation = replacement.navigation(in: owner)
        #expect(!replacementNavigation.owns(review, model: replacement, navigation: owner))
        replacementNavigation.show(replacement.path, model: replacement)
        #expect(!replacementNavigation.owns(review, model: replacement, navigation: owner))
    }

    @Test
    func anInternalPushKeepsTheOwnedPageButAPopOrForeignReplacementDoesNotRenderIt() {
        let model = makeModel()
        let owner = NavigationPathState()
        owner.append("cold-entry")
        let navigation = model.navigation(in: owner)
        model.path = [.signing, .review]
        navigation.show(model.path, model: model)
        let review = navigation.pageRoute(.review, at: 1)
        owner.append("internal-child")
        #expect(navigation.owns(review, model: model, navigation: owner))
        owner.removeLast()
        owner.removeLast()
        #expect(!navigation.owns(review, model: model, navigation: owner))
        navigation.reconcile(model: model)
        #expect(model.path == [.signing])
        #expect(navigation.owns(navigation.pageRoute(.signing, at: 0), model: model, navigation: owner))
        var foreign = NavigationPath()
        foreign.append("foreign-entry")
        foreign.append("foreign-page")
        owner.binding.wrappedValue = foreign
        #expect(!navigation.owns(navigation.pageRoute(.signing, at: 0), model: model, navigation: owner))
        navigation.show(model.path, model: model)
        #expect(owner.path == foreign)
    }

    @Test
    func cancellingTheActualModelRetiresItsRoutesWithoutMakingThemAvailableToAReplacement() {
        let model = makeModel()
        let owner = NavigationPathState()
        owner.append("cold-entry")
        let entry = owner.path
        let navigation = model.navigation(in: owner)
        model.path = [.signing]
        navigation.appear(model: model)
        let route = navigation.pageRoute(.signing, at: 0)
        #expect(navigation.owns(route, model: model, navigation: owner))
        model.cancel()
        #expect(owner.path == entry)
        #expect(!navigation.owns(route, model: model, navigation: owner))
        let replacement = makeModel()
        replacement.path = [.signing]
        let replacementNavigation = replacement.navigation(in: owner)
        replacementNavigation.appear(model: replacement)
        #expect(!replacementNavigation.owns(route, model: replacement, navigation: owner))
        #expect(replacementNavigation.owns(replacementNavigation.pageRoute(.signing, at: 0), model: replacement, navigation: owner))
    }

    @Test(.timeLimit(.minutes(1)))
    func theInstalledRootOwnsProjectionAndInitialBackWhileTheEntryPageIsCovered() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        service.currentWalletIdReply = { BsvColdWalletServiceMock.fixture.wallet.id }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: BsvColdWalletServiceMock.fixture.wallet.id, initialPage: .scanRequest(presentation: .resumed))
        await model.load()
        try #require(model.path == [.workspaceHome, .signing])
        let state = RootHostState(model: model)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.rootViewController = UIHostingController(rootView: RootHost(state: state))
        window.makeKeyAndVisible()
        defer {
            model.cancel()
            window.isHidden = true
            window.rootViewController = nil
        }
        await state.rootAppeared.wait()
        #expect(state.owner.path.isEmpty)
        #expect(state.installedModel == nil)
        state.owner.append(RootRoute.entry)
        await state.signingProjected.wait()
        try #require(state.owner.path.count == state.entry.count + 2)
        let navigation = model.navigation(in: state.owner)
        let home = navigation.pageRoute(.workspaceHome, at: 0)
        state.owner.removeLast()
        await state.homeReconciled.wait()
        #expect(model.path == [.workspaceHome])
        #expect(state.owner.path.count == state.entry.count + 1)
        #expect(navigation.owns(home, model: model, navigation: state.owner))
        #expect(state.initialBackCount == 0)
        state.owner.removeLast()
        #expect(!navigation.owns(home, model: model, navigation: state.owner))
        await state.closed.wait()
        #expect(state.initialBackCount == 1)
        #expect(state.owner.path.isEmpty)
        #expect(model.isCancelled)
    }

    fileprivate enum RootRoute: String, Hashable, Codable { case entry }

    @Observable @MainActor fileprivate final class RootHostState {
        let model: BsvColdWalletViewModel
        let owner = NavigationPathState()
        let entry: NavigationPath
        var isInstalled = false
        var initialBackCount = 0
        let rootAppeared = RootSignal()
        let signingProjected = RootSignal()
        let homeReconciled = RootSignal()
        let closed = RootSignal()

        init(model: BsvColdWalletViewModel) {
            self.model = model
            var path = NavigationPath()
            path.append(RootRoute.entry)
            entry = path
        }

        var installedModel: BsvColdWalletViewModel? {
            guard isInstalled, owner.path.count >= entry.count else { return nil }
            var prefix = owner.path
            prefix.removeLast(prefix.count - entry.count)
            return prefix == entry ? model : nil
        }

        func closeEntry() {
            guard owner.path == entry else { return }
            initialBackCount += 1
            owner.removeLast()
            closed.send()
        }
    }

    @MainActor fileprivate final class RootSignal {
        private let pair = AsyncStream<Void>.makeStream()
        func send() {
            pair.continuation.yield(())
        }

        func wait() async {
            var iterator = pair.stream.makeAsyncIterator()
            _ = await iterator.next()
        }
    }

    private struct RootHost: View {
        @Bindable var state: RootHostState

        var body: some View {
            NavigationStack(path: state.owner.binding) {
                Color.clear
                    .onAppear { state.rootAppeared.send() }
                    .navigationDestination(for: RootRoute.self) { _ in
                        BsvColdWalletScene(model: state.model, navigation: state.owner)
                            .onAppear { state.isInstalled = true }
                    }
                    .bsvColdWalletNavigation(model: state.installedModel, navigation: state.owner, onInitialPageBack: state.closeEntry)
            }
            .environment(\.scenePhase, .active)
            .onChange(of: state.owner.path) { _, path in
                if path.count == state.entry.count + 2 {
                    state.signingProjected.send()
                }
            }
            .onChange(of: state.model.path) { _, path in
                if path == [.workspaceHome] {
                    state.homeReconciled.send()
                }
            }
        }
    }

    private func makeModel() -> BsvColdWalletViewModel {
        BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .offline)
    }
}
