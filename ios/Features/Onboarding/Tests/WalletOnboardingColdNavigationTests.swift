// Copyright (c). Gem Wallet. All rights reserved.

@testable import Onboarding
import SwiftUI
import Testing

@MainActor
struct WalletOnboardingColdNavigationTests {
    @Test(arguments: [WalletOnboardingNavigation.Destination.coldCreate, .coldRecover, .coldConnect])
    func coldEntryRetainsItsOwnerForInternalPushAndBack(entry: WalletOnboardingNavigation.Destination) {
        var exits = 0
        let navigation = WalletOnboardingNavigation(onColdWalletDismiss: { exits += 1 })
        open(entry, navigation: navigation)
        let entryPath = navigation.path
        #expect(entryPath == NavigationPath([entry]))
        navigation.append(Review.page)
        #expect(navigation.path.count == 2)
        navigation.binding.wrappedValue = entryPath
        #expect(exits == 0)
        #expect(navigation.path == entryPath)
        navigation.binding.wrappedValue = NavigationPath()
        #expect(exits == 1)
        #expect(navigation.path.isEmpty)
    }

    @Test
    func replacingAColdEntryAtTheSameDepthCancelsTheOldOwner() {
        var exits = 0
        let navigation = WalletOnboardingNavigation(onColdWalletDismiss: { exits += 1 })
        navigation.openColdCreate()
        let replacement = NavigationPath([Review.page])
        navigation.binding.wrappedValue = replacement
        #expect(exits == 1)
        #expect(navigation.path == replacement)
        navigation.navigation.reset()
        #expect(exits == 1)
    }

    @Test
    func explicitExitCancelsOnceBeforeThePathDisappearsAndRejectsNewEntries() {
        let spy = ExitSpy()
        let navigation = WalletOnboardingNavigation(onColdWalletDismiss: {
            spy.pathCounts.append(spy.navigation?.path.count ?? -1)
            spy.navigation?.openColdConnect()
        })
        spy.navigation = navigation
        navigation.openColdRecover()
        let entryPath = navigation.path
        navigation.cancel()
        navigation.cancel()
        navigation.openColdCreate()
        navigation.openMultisig()
        #expect(spy.pathCounts == [1])
        #expect(navigation.path == entryPath)
        navigation.binding.wrappedValue = NavigationPath()
        #expect(spy.pathCounts == [1])
        navigation.append(Review.page)
        navigation.appendDistinct(Review.page)
        #expect(navigation.path.isEmpty)
    }

    @Test
    func aTruePopPreservesAReentrantlyOpenedColdEntry() {
        let spy = ExitSpy()
        let navigation = WalletOnboardingNavigation(onColdWalletDismiss: {
            spy.pathCounts.append(spy.navigation?.path.count ?? -1)
            if spy.pathCounts.count == 1 {
                spy.navigation?.openColdConnect()
            }
        })
        spy.navigation = navigation
        navigation.openColdRecover()
        navigation.binding.wrappedValue = NavigationPath()
        #expect(spy.pathCounts == [1])
        #expect(navigation.path.count == 2)
        navigation.openColdConnect()
        #expect(navigation.path.count == 2)
        navigation.binding.wrappedValue = NavigationPath()
        #expect(spy.pathCounts == [1, 2])
        #expect(navigation.path.isEmpty)
    }

    @Test
    func onlyOneMethodEntryOwnsCancellationUntilReturningToMethods() {
        var coldExits = 0
        var collectiveExits = 0
        let navigation = WalletOnboardingNavigation(onMultisigDismiss: { collectiveExits += 1 }, onColdWalletDismiss: { coldExits += 1 })
        navigation.openColdCreate()
        navigation.openColdRecover()
        navigation.openColdConnect()
        navigation.openMultisig()
        #expect(navigation.path == NavigationPath([WalletOnboardingNavigation.Destination.coldCreate]))
        navigation.binding.wrappedValue = NavigationPath()
        #expect(coldExits == 1 && collectiveExits == 0)
        navigation.openMultisig()
        navigation.openColdCreate()
        #expect(navigation.path == NavigationPath([WalletOnboardingNavigation.Destination.multisig]))
        navigation.binding.wrappedValue = NavigationPath()
        #expect(coldExits == 1 && collectiveExits == 1)
    }

    private func open(_ entry: WalletOnboardingNavigation.Destination, navigation: WalletOnboardingNavigation) {
        switch entry {
        case .coldCreate: navigation.openColdCreate()
        case .coldRecover: navigation.openColdRecover()
        case .coldConnect: navigation.openColdConnect()
        case .multisig: navigation.openMultisig()
        }
    }

    private enum Review: String, Hashable, Codable { case page }

    @MainActor
    private final class ExitSpy {
        weak var navigation: WalletOnboardingNavigation?
        var pathCounts: [Int] = []
    }
}
