// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import Observation
@testable import Onboarding
import SwiftUI
import Testing
import UIKit

@MainActor
@Suite(.serialized)
struct WalletOnboardingNavigationTests {
    @Test(.timeLimit(.minutes(1)))
    func collectiveEntrySurvivesInternalPushRebuildAndBackUntilItIsPopped() async throws {
        let exit = ExitSpy()
        let navigation = WalletOnboardingNavigation(onMultisigDismiss: { exit.record() })
        exit.navigation = navigation
        let state = HostState(navigation: navigation)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.rootViewController = UIHostingController(rootView: Host(state: state))
        window.makeKeyAndVisible()
        defer {
            window.isHidden = true
            window.rootViewController = nil
        }
        await state.rootAppeared.wait()
        navigation.openMultisig()
        await state.collectiveAppeared.wait()
        let entryPath = navigation.path
        #expect(entryPath.count == 1)
        #expect(exit.pathCounts.isEmpty)

        let pushReview = try #require(state.pushReview)
        pushReview()
        await state.reviewAppeared.wait()
        #expect(navigation.path.count == 2)
        #expect(exit.pathCounts.isEmpty)
        state.revision += 1
        await state.rebuilt.wait()
        #expect(navigation.path.count == 2)
        #expect(exit.pathCounts.isEmpty)

        navigation.binding.wrappedValue = entryPath
        await state.collectiveAppeared.wait()
        #expect(navigation.path == entryPath)
        #expect(exit.pathCounts.isEmpty)
        navigation.binding.wrappedValue = NavigationPath()
        await state.rootAppeared.wait()
        #expect(navigation.path.isEmpty)
        #expect(exit.pathCounts == [1])
        navigation.binding.wrappedValue = NavigationPath()
        #expect(exit.pathCounts == [1])
    }

    @Test(arguments: ReentrantExit.allCases)
    func trueExitCancelsBeforeAssignmentAndPreservesReentrantNavigation(action: ReentrantExit) {
        let exit = ExitSpy()
        let replacement = NavigationPath([Route.review])
        let navigation = WalletOnboardingNavigation(onMultisigDismiss: {
            exit.record()
            guard exit.pathCounts.count == 1, let navigation = exit.navigation else { return }
            navigation.binding.wrappedValue = NavigationPath()
            switch action {
            case .clear: break
            case .replace: navigation.binding.wrappedValue = replacement
            case .reopen: navigation.openMultisig()
            }
        })
        exit.navigation = navigation
        navigation.openMultisig()
        navigation.binding.wrappedValue = NavigationPath()

        #expect(exit.pathCounts == [1])
        switch action {
        case .clear: #expect(navigation.path.isEmpty)
        case .replace: #expect(navigation.path == replacement)
        case .reopen: #expect(navigation.path.count == 1)
        }
        navigation.binding.wrappedValue = NavigationPath()
        #expect(navigation.path.isEmpty)
        #expect(exit.pathCounts == (action == .reopen ? [1, 1] : [1]))
    }

    @Test
    func replacingEntryAtSameDepthCancelsBeforeReplacement() {
        let exit = ExitSpy()
        let navigation = WalletOnboardingNavigation(onMultisigDismiss: { exit.record() })
        exit.navigation = navigation
        navigation.openMultisig()
        let replacement = NavigationPath([Route.review])

        navigation.binding.wrappedValue = replacement

        #expect(exit.pathCounts == [1])
        #expect(navigation.path == replacement)
    }

    @Test
    func sharedNavigationRemovalCancelsEntry() {
        let exit = ExitSpy()
        let navigation = WalletOnboardingNavigation(onMultisigDismiss: { exit.record() })
        exit.navigation = navigation
        navigation.openMultisig()

        navigation.navigation.reset()

        #expect(exit.pathCounts == [1])
        #expect(navigation.path.isEmpty)
    }

    @Test
    func reentrantReopenSurvivesOriginalRemoval() {
        let exit = ExitSpy()
        let navigation = WalletOnboardingNavigation(onMultisigDismiss: {
            exit.record()
            if exit.pathCounts.count == 1 {
                exit.navigation?.openMultisig()
            }
        })
        exit.navigation = navigation
        navigation.openMultisig()

        navigation.binding.wrappedValue = NavigationPath()

        #expect(exit.pathCounts == [1])
        #expect(navigation.path.count == 2)
        navigation.openMultisig()
        #expect(navigation.path.count == 2)
        navigation.navigation.removeLast()
        #expect(exit.pathCounts == [1, 2])
        #expect(navigation.path.count == 1)
    }

    @Test
    func distinctCreateAppendAndHashableImportAppendRemainCompatible() {
        let navigation = WalletOnboardingNavigation()

        navigation.appendDistinct(Route.review)
        navigation.appendDistinct(Route.review)
        #expect(navigation.path.count == 1)
        navigation.append(ImportRoute.input)
        navigation.append(ImportRoute.input)
        #expect(navigation.path.count == 3)
        #expect(navigation.path.codable == nil)
    }

    enum ReentrantExit: CaseIterable, Equatable {
        case clear, replace, reopen
    }

    fileprivate enum Route: String, Hashable, Codable {
        case review
    }

    private enum ImportRoute: Hashable {
        case input
    }

    @MainActor
    private final class ExitSpy {
        weak var navigation: WalletOnboardingNavigation?
        var pathCounts: [Int] = []

        func record() {
            if let navigation {
                pathCounts.append(navigation.path.count)
            }
        }
    }

    @Observable
    @MainActor
    fileprivate final class HostState {
        let navigation: WalletOnboardingNavigation
        var revision = 0
        @ObservationIgnored var pushReview: (() -> Void)?
        let rootAppeared = Signal()
        let collectiveAppeared = Signal()
        let reviewAppeared = Signal()
        let rebuilt = Signal()

        init(navigation: WalletOnboardingNavigation) {
            self.navigation = navigation
        }
    }

    private struct Host: View {
        @Bindable var state: HostState

        var body: some View {
            NavigationStack(path: state.navigation.binding) {
                Text("Methods")
                    .navigationTitle("Methods")
                    .onAppear { state.rootAppeared.send() }
                    .navigationDestination(for: WalletOnboardingNavigation.Destination.self) { _ in
                        CollectiveRoot(state: state, path: state.navigation.binding)
                    }
            }
            .environment(\.locale, Locale(identifier: state.revision.isMultiple(of: 2) ? "en_US" : "zh_Hans"))
            .onChange(of: state.revision) { _, _ in state.rebuilt.send() }
        }
    }

    private struct CollectiveRoot: View {
        let state: HostState
        @Binding var path: NavigationPath

        var body: some View {
            Text("Collective form")
                .navigationTitle("Collective form")
                .onAppear {
                    state.pushReview = { path.append(Route.review) }
                    state.collectiveAppeared.send()
                }
                .navigationDestination(for: Route.self) { _ in
                    Text("Review")
                        .navigationTitle("Review")
                        .onAppear { state.reviewAppeared.send() }
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
}
