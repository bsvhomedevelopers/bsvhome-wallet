// Copyright (c). Gem Wallet. All rights reserved.

@testable import Components
import SwiftUI
import Testing

@MainActor
struct NavigationPathStateTests {
    @Test
    func initialState() {
        let state = NavigationPathState()

        #expect(state.isEmpty == true)
        #expect(state.count == 0)
    }

    @Test
    func appendElement() {
        let state = NavigationPathState()

        #expect(state.append(TestScene(id: "a")) == true)
        #expect(state.count == 1)
    }

    @Test
    func appendDuplicateElement() {
        let state = NavigationPathState()
        state.append(TestScene(id: "a"))

        #expect(state.append(TestScene(id: "a")) == false)
        #expect(state.count == 1)
    }

    @Test
    func appendDifferentElement() {
        let state = NavigationPathState()
        state.append(TestScene(id: "a"))

        #expect(state.append(TestScene(id: "b")) == true)
        #expect(state.count == 2)
    }

    @Test
    func appendDifferentType() {
        let state = NavigationPathState()
        state.append(TestScene(id: "a"))

        #expect(state.append(OtherScene(id: "a")) == true)
        #expect(state.count == 2)
    }

    @Test
    func setPath() {
        let state = NavigationPathState()

        state.setPath([TestScene(id: "a"), TestScene(id: "b"), TestScene(id: "c")])

        #expect(state.count == 3)
    }

    @Test
    func setPathOverwrites() {
        let state = NavigationPathState()
        state.append(TestScene(id: "x"))

        state.setPath([TestScene(id: "a"), TestScene(id: "b")])

        #expect(state.count == 2)
    }

    @Test
    func reset() {
        let state = NavigationPathState()
        state.append(TestScene(id: "a"))

        state.reset()

        #expect(state.isEmpty == true)
    }

    @Test
    func removeLast() {
        let state = NavigationPathState()
        state.append(TestScene(id: "a"))
        state.append(TestScene(id: "b"))

        state.removeLast()

        #expect(state.count == 1)
    }

    @Test
    func removeLastOnEmptyPath() {
        let state = NavigationPathState()

        state.removeLast()

        #expect(state.isEmpty == true)
    }

    @Test
    func removeLastClampsToAvailableElements() {
        let state = NavigationPathState()
        state.append(TestScene(id: "a"))

        state.removeLast(2)

        #expect(state.isEmpty == true)
    }

    @Test
    func removalObserverPreservesInternalPushAndReturn() {
        let state = NavigationPathState()
        state.append(TestScene(id: "feature"))
        let entry = state.path
        var removals = 0
        state.observeRemoval { removals += 1 }

        state.append(TestScene(id: "review"))
        state.binding.wrappedValue = entry

        #expect(state.path == entry)
        #expect(removals == 0)
        state.removeLast()
        #expect(removals == 1)
    }

    @Test(arguments: RemovalMutation.allCases)
    func removalObserverRunsSynchronouslyBeforeEveryRemoval(mutation: RemovalMutation) {
        let state = NavigationPathState()
        state.setPath([TestScene(id: "root"), TestScene(id: "feature")])
        var pathsAtRemoval: [NavigationPath] = []
        state.observeRemoval { pathsAtRemoval.append(state.path) }
        state.append(TestScene(id: "review"))
        let beforeRemoval = state.path

        switch mutation {
        case .binding: state.binding.wrappedValue = NavigationPath([TestScene(id: "root")])
        case .setPath: state.setPath([TestScene(id: "root")])
        case .reset: state.reset()
        case .removeLast: state.removeLast(2)
        }

        #expect(pathsAtRemoval == [beforeRemoval])
        #expect(state.count == (mutation == .reset ? 0 : 1))
        state.reset()
        #expect(pathsAtRemoval.count == 1)
    }

    @Test(arguments: ReplacementMutation.allCases)
    func sameCountReplacementRemovesObservedFeature(mutation: ReplacementMutation) {
        let state = NavigationPathState()
        state.append(TestScene(id: "feature"))
        let beforeReplacement = state.path
        let replacement = NavigationPath([OtherScene(id: "feature")])
        var pathsAtRemoval: [NavigationPath] = []
        state.observeRemoval { pathsAtRemoval.append(state.path) }

        switch mutation {
        case .binding: state.binding.wrappedValue = replacement
        case .setPath: state.setPath([OtherScene(id: "feature")])
        }

        #expect(pathsAtRemoval == [beforeReplacement])
        #expect(state.path == replacement)
    }

    @Test
    func removedObserverDoesNotReceiveExit() {
        let state = NavigationPathState()
        state.append(TestScene(id: "feature"))
        var removals = 0
        let id = state.observeRemoval { removals += 1 }

        state.removeRemovalObserver(id)
        state.removeRemovalObserver(id)
        state.reset()

        #expect(removals == 0)
    }

    @Test
    func removalObserverSupportsNonCodableNavigation() {
        let state = NavigationPathState()
        let entry = NavigationPath([NonCodableScene(id: "feature")])
        state.binding.wrappedValue = entry
        var removals = 0
        state.observeRemoval { removals += 1 }
        var review = entry
        review.append(NonCodableScene(id: "review"))

        state.binding.wrappedValue = review
        state.binding.wrappedValue = entry
        #expect(removals == 0)
        state.reset()
        #expect(removals == 1)
    }

    @Test(arguments: ReentrantMutation.allCases)
    func removalDoesNotOverwriteReentrantNavigation(mutation: ReentrantMutation) {
        let state = NavigationPathState()
        state.append(TestScene(id: "feature"))
        var removals = 0
        state.observeRemoval {
            removals += 1
            switch mutation {
            case .binding: state.binding.wrappedValue = NavigationPath([OtherScene(id: "replacement")])
            case .setPath: state.setPath([OtherScene(id: "replacement")])
            case .reset: state.reset()
            case .removeLast: state.removeLast()
            case .append: state.append(OtherScene(id: "replacement"))
            }
        }

        state.reset()

        #expect(removals == 1)
        switch mutation {
        case .binding, .setPath:
            #expect(state.path == NavigationPath([OtherScene(id: "replacement")]))
        case .reset, .removeLast:
            #expect(state.isEmpty)
        case .append:
            var expected = NavigationPath([TestScene(id: "feature")])
            expected.append(OtherScene(id: "replacement"))
            #expect(state.path == expected)
        }
    }

    @Test
    func reentrantReplacementCanInstallNewRemovalObserver() {
        let state = NavigationPathState()
        state.append(TestScene(id: "feature"))
        var oldRemovals = 0
        var newRemovals = 0
        state.observeRemoval {
            oldRemovals += 1
            state.setPath([OtherScene(id: "replacement")])
            state.observeRemoval { newRemovals += 1 }
        }

        state.reset()

        #expect(state.path == NavigationPath([OtherScene(id: "replacement")]))
        #expect(oldRemovals == 1)
        #expect(newRemovals == 0)
        state.reset()
        #expect(oldRemovals == 1)
        #expect(newRemovals == 1)
    }

    enum RemovalMutation: CaseIterable {
        case binding, setPath, reset, removeLast
    }

    enum ReplacementMutation: CaseIterable {
        case binding, setPath
    }

    enum ReentrantMutation: CaseIterable {
        case binding, setPath, reset, removeLast, append
    }
}

private struct TestScene: Hashable, Codable {
    let id: String
}

private struct OtherScene: Hashable, Codable {
    let id: String
}

private struct NonCodableScene: Hashable {
    let id: String
}
