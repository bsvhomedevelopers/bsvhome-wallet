@testable import BsvColdWallet
import Components
import Foundation
@testable import BsvHome
import BsvHomeCore
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import GRDB
@testable import Onboarding
import Primitives
import Store
import SwiftUI
import Testing

@MainActor
struct ColdWalletSelectionResumeTests {
    @Test
    func aRealConsumedCoreSelectionResumesTheSameSceneAfterContextRecreation() async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let scene = Scenes.BsvColdWallet(walletId: fixture.a.id, entry: .signing)
        let navigation = NavigationPathState()
        navigation.append(scene)
        let resume = ColdWalletSelectionResume(scene: scene)
        let first = fixture.context(id: scene.id, walletId: resume.walletId(for: scene))
        var active: BsvColdWalletContext? = first
        resume.observe(context: first, navigation: navigation, entryPath: navigation.path) { active === $0 }
        #expect(resume.walletId(for: scene) == fixture.a.id)
        try await fixture.service(in: first).selectWallet(walletId: fixture.b.id)
        #expect(try first.consumeWalletSelection(walletId: fixture.b.id))
        #expect(try !first.consumeWalletSelection(walletId: fixture.b.id))
        #expect(resume.walletId(for: scene) == fixture.b.id)

        let recreatedView = ColdWalletSelectionResume(scene: scene)
        recreatedView.observe(context: first, navigation: navigation, entryPath: navigation.path) { active === $0 }
        #expect(recreatedView.walletId(for: scene) == fixture.b.id)
        first.cancel()
        active = nil
        let replacement = fixture.context(id: scene.id, walletId: recreatedView.walletId(for: scene))
        active = replacement
        recreatedView.observe(context: replacement, navigation: navigation, entryPath: navigation.path) { active === $0 }
        #expect(recreatedView.walletId(for: scene) == fixture.b.id)
        #expect(try fixture.session.getCurrentWalletId() == fixture.b.id)
        #expect(try !replacement.consumeWalletSelection(walletId: fixture.b.id))
        #expect(fixture.ports.calls == 0)
        replacement.cancel()
    }

    @Test
    func scanRequestAdmitsOnceAndKeepsTheActualCoreWalletBWhenItsContextRebuilds() async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let scene = Scenes.BsvColdWallet(walletId: fixture.a.id, entry: .scanRequest)
        let owner = NavigationPathState()
        owner.append(scene)
        let entryPath = owner.path
        let resume = ColdWalletSelectionResume(scene: scene)
        #expect(resume.initialPage(for: scene) == .scanRequest(presentation: .initial))
        #expect(resume.initialPage(for: scene) == .scanRequest(presentation: .initial))
        let context = fixture.context(id: scene.id, walletId: fixture.a.id, entry: resume.initialPage(for: scene))
        var active: BsvColdWalletContext?
        resume.admitted(context: context, navigation: owner, entryPath: entryPath) { active === $0 }
        #expect(resume.initialPage(for: scene) == .scanRequest(presentation: .initial))
        active = context
        resume.admitted(context: context, navigation: owner, entryPath: entryPath) { active === $0 }
        #expect(resume.initialPage(for: scene) == .scanRequest(presentation: .resumed))
        owner.append("rendered-workspace")
        let recreatedView = ColdWalletSelectionResume(scene: scene)
        recreatedView.observe(context: context, navigation: owner, entryPath: entryPath) { active === $0 }
        #expect(recreatedView.initialPage(for: scene) == .scanRequest(presentation: .resumed))
        try await fixture.service(in: context).selectWallet(walletId: fixture.b.id)
        #expect(try context.consumeWalletSelection(walletId: fixture.b.id))
        #expect(recreatedView.walletId(for: scene) == fixture.b.id)
        context.cancelForNavigationRemoval()
        active = nil
        let replacement = fixture.context(id: scene.id, walletId: recreatedView.walletId(for: scene), entry: recreatedView.initialPage(for: scene))
        defer { replacement.cancel() }
        active = replacement
        recreatedView.observe(context: replacement, navigation: owner, entryPath: entryPath) { active === $0 }
        #expect(recreatedView.initialPage(for: scene) == .scanRequest(presentation: .resumed))
        #expect(recreatedView.walletId(for: scene) == fixture.b.id)
        #expect(try fixture.session.getCurrentWalletId() == fixture.b.id)
        #expect(fixture.ports.calls == 0)
    }

    @Test
    func aForeignOrCancelledContextCannotConsumeAnotherScanRequestEntry() async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let scene = Scenes.BsvColdWallet(walletId: fixture.a.id, entry: .scanRequest)
        let owner = NavigationPathState()
        owner.append(scene)
        let entryPath = owner.path
        let resume = ColdWalletSelectionResume(scene: scene)
        let context = fixture.context(id: UUID(), walletId: fixture.a.id)
        defer { context.cancel() }
        resume.admitted(context: context, navigation: owner, entryPath: entryPath) { _ in true }
        #expect(resume.initialPage(for: scene) == .scanRequest(presentation: .initial))
        let matching = fixture.context(id: scene.id, walletId: fixture.a.id)
        owner.setPath(["foreign"])
        resume.admitted(context: matching, navigation: owner, entryPath: entryPath) { _ in true }
        #expect(resume.initialPage(for: scene) == .scanRequest(presentation: .initial))
        owner.binding.wrappedValue = entryPath
        matching.cancelForNavigationRemoval()
        resume.admitted(context: matching, navigation: owner, entryPath: entryPath) { _ in true }
        #expect(resume.initialPage(for: scene) == .scanRequest(presentation: .initial))
        #expect(fixture.ports.calls == 0)
    }

    @Test
    func manualCurrentChangesAndFailedSelectionPersistenceDoNotMintResumeClaims() async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let scene = Scenes.BsvColdWallet(walletId: fixture.a.id, entry: .signing)
        let navigation = NavigationPathState()
        navigation.append(scene)
        let resume = ColdWalletSelectionResume(scene: scene)
        let context = fixture.context(id: scene.id, walletId: fixture.a.id)
        defer { context.cancel() }
        resume.observe(context: context, navigation: navigation, entryPath: navigation.path) { $0 === context }
        try fixture.session.setCurrentWalletId(walletId: fixture.b.id)
        #expect(try !context.consumeWalletSelection(walletId: fixture.b.id))
        #expect(resume.walletId(for: scene) == fixture.a.id)
        try fixture.session.setCurrentWalletId(walletId: fixture.a.id)
        fixture.preferences.failWrites(with: .Store(msg: "Selection fixture write rejected"))
        await #expect(throws: BsvHomeServiceError.self) { try await fixture.service(in: context).selectWallet(walletId: fixture.b.id) }
        #expect(try !context.consumeWalletSelection(walletId: fixture.b.id))
        #expect(resume.walletId(for: scene) == fixture.a.id)
        #expect(try fixture.session.getCurrentWalletId() == fixture.a.id)
        #expect(fixture.ports.calls == 0)
    }

    @Test
    func explicitPairingKeepsItsOriginalSourceEvenWhenAnotherCoreSelectionWasClaimed() async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let scene = Scenes.BsvColdWallet(walletId: fixture.a.id, entry: .pairing)
        let navigation = NavigationPathState()
        navigation.append(scene)
        let resume = ColdWalletSelectionResume(scene: scene)
        let context = fixture.context(id: scene.id, walletId: fixture.a.id)
        defer { context.cancel() }
        resume.observe(context: context, navigation: navigation, entryPath: navigation.path) { $0 === context }
        try await fixture.service(in: context).selectWallet(walletId: fixture.b.id)
        #expect(try context.consumeWalletSelection(walletId: fixture.b.id))
        #expect(resume.walletId(for: scene) == fixture.a.id)
        #expect(fixture.ports.calls == 0)
    }

    @Test
    func aClaimedOldSceneCannotOverwriteAnEqualDepthForeignReplacementOrInactiveOwner() async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let scene = Scenes.BsvColdWallet(walletId: fixture.a.id, entry: .signing)
        let foreign = Scenes.BsvColdWallet(id: scene.id, walletId: fixture.c.id, entry: .signing)
        let navigation = NavigationPathState()
        navigation.append(scene)
        let entryPath = navigation.path
        let resume = ColdWalletSelectionResume(scene: scene)
        let context = fixture.context(id: scene.id, walletId: fixture.a.id)
        defer { context.cancel() }
        var active: BsvColdWalletContext? = context
        resume.observe(context: context, navigation: navigation, entryPath: entryPath) { active === $0 }
        try await fixture.service(in: context).selectWallet(walletId: fixture.b.id)
        navigation.setPath([foreign])
        let foreignPath = navigation.path
        #expect(try context.consumeWalletSelection(walletId: fixture.b.id))
        #expect(resume.walletId(for: scene) == fixture.a.id)
        #expect(resume.walletId(for: foreign) == fixture.c.id)
        #expect(navigation.path == foreignPath)
        active = nil
        navigation.setPath([scene])
        try await fixture.service(in: context).selectWallet(walletId: fixture.c.id)
        #expect(try context.consumeWalletSelection(walletId: fixture.c.id))
        #expect(resume.walletId(for: scene) == fixture.a.id)
        #expect(navigation.path == entryPath)
        #expect(fixture.ports.calls == 0)
    }

    @Test(arguments: EntryExit.allCases)
    func wholeEntryRemovalInvalidatesBeforeAssignmentWithoutReplacingTheIncomingPath(exit: EntryExit) async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let scene = Scenes.BsvColdWallet(walletId: fixture.a.id, entry: .signing)
        let owner = NavigationPathState()
        owner.append(scene)
        let context = fixture.context(id: scene.id, walletId: fixture.a.id)
        let foreignContext = fixture.context(id: UUID(), walletId: fixture.a.id)
        defer { foreignContext.cancel() }
        let service = try fixture.service(in: context)
        let generation = service.captureGeneration()
        let foreignService = try fixture.service(in: foreignContext)
        let foreignGeneration = foreignService.captureGeneration()
        var observedPaths: [NavigationPath] = []
        owner.observeRemoval {
            observedPaths.append(owner.path)
            context.cancelForNavigationRemoval()
        }
        let navigation = BsvColdWalletNavigation(navigation: owner)
        context.model.attachNavigationRetirement { navigation.retire() }
        context.model.path = [.signing, .review]
        navigation.show(context.model.path, model: context.model)
        let rendered = owner.path
        let replacement = switch exit {
        case .back: NavigationPath()
        case .replace: NavigationPath(["foreign-cold-entry"])
        }

        owner.binding.wrappedValue = replacement

        #expect(observedPaths == [rendered])
        #expect(owner.path == replacement)
        #expect(context.model.isCancelled)
        #expect(context.model.path.isEmpty)
        #expect(service.captureGeneration() != generation)
        #expect(!foreignContext.model.isCancelled)
        #expect(foreignService.captureGeneration() == foreignGeneration)
        context.cancel()
        navigation.reconcile(model: context.model)
        #expect(owner.path == replacement)
        #expect(observedPaths.count == 1)
        #expect(fixture.ports.calls == 0)
    }

    @Test(arguments: [WalletOnboardingNavigation.Destination.coldCreate, .coldRecover, .coldConnect])
    func actualOnboardingEntryRemovalKeepsItsForeignReplacementAfterColdCancellation(entry: WalletOnboardingNavigation.Destination) async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let context = fixture.context(id: UUID(), walletId: fixture.a.id)
        let service = try fixture.service(in: context)
        let generation = service.captureGeneration()
        let onboarding = WalletOnboardingNavigation(onColdWalletDismiss: { context.cancelForNavigationRemoval() })
        switch entry {
        case .coldCreate: onboarding.openColdCreate()
        case .coldRecover: onboarding.openColdRecover()
        case .coldConnect: onboarding.openColdConnect()
        case .multisig: Issue.record("A cold entry is required")
        }
        let navigation = BsvColdWalletNavigation(navigation: onboarding.navigation)
        context.model.attachNavigationRetirement { navigation.retire() }
        context.model.path = [.recover, .importReview]
        navigation.show(context.model.path, model: context.model)
        let replacement = NavigationPath(["foreign-onboarding-entry"])

        onboarding.binding.wrappedValue = replacement

        #expect(onboarding.path == replacement)
        #expect(context.model.isCancelled)
        #expect(service.captureGeneration() != generation)
        onboarding.cancel()
        context.cancel()
        #expect(onboarding.path == replacement)
        #expect(fixture.ports.calls == 0)
    }

    @Test
    func explicitCancellationStillRetiresOnlyItsOwnedSuffix() async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let owner = NavigationPathState()
        let scene = Scenes.BsvColdWallet(walletId: fixture.a.id, entry: .signing)
        owner.append(scene)
        let entryPath = owner.path
        let context = fixture.context(id: scene.id, walletId: fixture.a.id)
        let service = try fixture.service(in: context)
        let generation = service.captureGeneration()
        var entryExits = 0
        owner.observeRemoval { entryExits += 1 }
        let navigation = BsvColdWalletNavigation(navigation: owner)
        context.model.attachNavigationRetirement { navigation.retire() }
        context.model.path = [.signing, .review]
        navigation.show(context.model.path, model: context.model)
        try #require(owner.path.count == entryPath.count + 2)

        context.cancel()

        #expect(owner.path == entryPath)
        #expect(entryExits == 0)
        #expect(context.model.isCancelled)
        #expect(service.captureGeneration() != generation)
        #expect(fixture.ports.calls == 0)
    }

    @Test(arguments: InitialEntry.allCases)
    func backingOutOfTheActualInitialPageClosesOnlyItsEntryAfterReconciliation(entry: InitialEntry) async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let owner = NavigationPathState()
        owner.append("settings-parent")
        let parent = owner.path
        let scene = Scenes.BsvColdWallet(walletId: fixture.a.id, entry: entry.scene)
        owner.append(scene)
        let entryPath = owner.path
        let context = fixture.context(id: scene.id, walletId: fixture.a.id, entry: entry.entry)
        let service = try fixture.service(in: context)
        let generation = service.captureGeneration()
        var active: BsvColdWalletContext? = context
        var backs = 0
        let onBack: @MainActor () -> Void = {
            guard active === context, owner.path == entryPath else { return }
            backs += 1
            active = nil
            context.cancelForNavigationRemoval()
            owner.binding.wrappedValue = parent
        }
        let navigation = BsvColdWalletNavigation(navigation: owner)
        context.model.attachNavigationRetirement { navigation.retire() }
        context.model.path = [entry.route]
        navigation.show(context.model.path, model: context.model)
        if let child = entry.child {
            context.model.path.append(child)
            navigation.show(context.model.path, model: context.model)
            let childPath = owner.path
            navigation.appear(model: context.model, onInitialPageBack: onBack)
            #expect(owner.path == childPath)
            #expect(backs == 0)
            #expect(!context.model.isCancelled)
            owner.removeLast()
            navigation.reconcile(model: context.model, onInitialPageBack: onBack)
            #expect(backs == 0)
            #expect(!context.model.isCancelled)
            #expect(context.model.path == [entry.route])
        }

        owner.removeLast()
        #expect(owner.path == entryPath)
        #expect(backs == 0)
        #expect(!context.model.isCancelled)
        navigation.reconcile(model: context.model, onInitialPageBack: onBack)

        #expect(owner.path == parent)
        #expect(backs == 1)
        #expect(active == nil)
        #expect(context.model.isCancelled)
        #expect(service.captureGeneration() != generation)
        navigation.reconcile(model: context.model, onInitialPageBack: onBack)
        context.cancel()
        #expect(backs == 1)
        #expect(owner.path == parent)
        #expect(fixture.ports.calls == 0)
    }

    @Test(arguments: InitialOnboardingEntry.allCases)
    func initialOnboardingBackReturnsToTheRealMethodsPathWithoutCompleting(entry: InitialOnboardingEntry) async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        var successes = 0
        let context = fixture.context(id: UUID(), walletId: fixture.a.id, entry: entry.entry, onComplete: { successes += 1 })
        var active: BsvColdWalletContext? = context
        var dismissals = 0
        let onboarding = WalletOnboardingNavigation(onColdWalletDismiss: {
            dismissals += 1
            guard active === context else { return }
            active = nil
            context.cancelForNavigationRemoval()
        })
        onboarding.appendDistinct("methods-parent")
        let parent = onboarding.path
        switch entry {
        case .create: onboarding.openColdCreate()
        case .recover: onboarding.openColdRecover()
        case .connect: onboarding.openColdConnect()
        }
        let entryPath = onboarding.path
        let navigation = BsvColdWalletNavigation(navigation: onboarding.navigation)
        context.model.attachNavigationRetirement { navigation.retire() }
        context.model.path = [entry.route]
        navigation.show(context.model.path, model: context.model)
        let service = try fixture.service(in: context)
        let generation = service.captureGeneration()
        var backs = 0

        onboarding.navigation.removeLast()
        #expect(onboarding.path == entryPath)
        #expect(dismissals == 0)
        navigation.reconcile(model: context.model, onInitialPageBack: {
            guard active === context, onboarding.path == entryPath else { return }
            backs += 1
            active = nil
            context.cancelForNavigationRemoval()
            onboarding.binding.wrappedValue = parent
        })

        #expect(onboarding.path == parent)
        #expect(backs == 1)
        #expect(dismissals == 1)
        #expect(successes == 0)
        #expect(context.model.isCancelled)
        #expect(service.captureGeneration() != generation)
        context.cancel()
        #expect(onboarding.path == parent)
        #expect(fixture.ports.calls == 0)
    }

    @Test(arguments: InitialBackControl.allCases)
    func initialBackDoesNotAdoptHomeMissingCallbacksOrForeignPaths(control: InitialBackControl) async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let owner = NavigationPathState()
        owner.append("current-entry")
        let base = owner.path
        let entry: BsvColdWalletEntry = switch control {
        case .home: .home
        case .mismatchedInitial: .pairing
        case .missingCallback, .foreignEntry, .foreignChild: .send
        }
        let context = fixture.context(id: UUID(), walletId: fixture.a.id, entry: entry)
        defer { context.cancel() }
        let navigation = BsvColdWalletNavigation(navigation: owner)
        context.model.attachNavigationRetirement { navigation.retire() }
        context.model.path = [.send]
        navigation.show(context.model.path, model: context.model)
        let incoming: NavigationPath
        switch control {
        case .home, .missingCallback, .mismatchedInitial:
            owner.removeLast()
            incoming = base
        case .foreignEntry:
            incoming = NavigationPath(["foreign-entry"])
            owner.binding.wrappedValue = incoming
        case .foreignChild:
            var replacement = base
            replacement.append("foreign-child")
            incoming = replacement
            owner.binding.wrappedValue = incoming
        }
        var backs = 0
        if control == .missingCallback {
            navigation.reconcile(model: context.model)
        } else {
            navigation.reconcile(model: context.model, onInitialPageBack: { backs += 1 })
        }
        #expect(backs == 0)
        #expect(owner.path == incoming)
        #expect(context.model.isCancelled == (control == .foreignEntry))
        #expect(fixture.ports.calls == 0)
    }

    @Test
    func completingTheActualEntryDoesNotInvokeInitialBackCancellation() async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let owner = NavigationPathState()
        owner.append("methods-parent")
        let parent = owner.path
        let scene = Scenes.BsvColdWallet(walletId: fixture.a.id, entry: .send)
        owner.append(scene)
        var successes = 0
        let context = fixture.context(id: scene.id, walletId: fixture.a.id, entry: .send, onComplete: {
            successes += 1
            owner.binding.wrappedValue = parent
        })
        owner.observeRemoval { context.cancelForNavigationRemoval() }
        let navigation = BsvColdWalletNavigation(navigation: owner)
        context.model.attachNavigationRetirement { navigation.retire() }
        context.model.path = [.send]
        navigation.show(context.model.path, model: context.model)
        var backs = 0

        context.model.complete()
        navigation.reconcile(model: context.model, onInitialPageBack: { backs += 1 })

        #expect(successes == 1)
        #expect(backs == 0)
        #expect(owner.path == parent)
        #expect(context.model.isCancelled)
        #expect(fixture.ports.calls == 0)
    }

    @Test(arguments: InitialAppearance.allCases)
    func appearingBeforeThePathChangeCallbackDoesNotRestoreThePoppedPage(appearance: InitialAppearance) async throws {
        let fixture = try await SelectionFixture.make()
        defer { try? FileManager.default.removeItem(at: fixture.directory) }
        let owner = NavigationPathState()
        let parent = owner.path
        if appearance == .directEntry {
            owner.append(Scenes.BsvColdWallet(walletId: fixture.a.id, entry: .signing))
        }
        let base = owner.path
        let context = fixture.context(id: UUID(), walletId: fixture.a.id, entry: appearance == .directEntry ? .signing : .home)
        defer { context.cancel() }
        context.model.path = appearance == .directEntry ? [.signing] : [.pairing]
        let navigation = BsvColdWalletNavigation(navigation: owner)
        var backs = 0
        let onBack: (@MainActor () -> Void)? = switch appearance {
        case .workspace: nil
        case .directEntry:
            {
                guard owner.path == base else { return }
                backs += 1
                context.cancelForNavigationRemoval()
                owner.binding.wrappedValue = parent
            }
        }
        navigation.appear(model: context.model, onInitialPageBack: onBack)
        try #require(owner.path.count == base.count + 1)

        owner.removeLast()
        #expect(owner.path == base)
        navigation.appear(model: context.model, onInitialPageBack: onBack)
        navigation.reconcile(model: context.model, onInitialPageBack: onBack)

        #expect(owner.path == parent)
        #expect(context.model.path.isEmpty)
        #expect(backs == (appearance == .directEntry ? 1 : 0))
        #expect(context.model.isCancelled == (appearance == .directEntry))
        #expect(fixture.ports.calls == 0)
    }

    enum InitialAppearance: CaseIterable {
        case workspace, directEntry
    }

    enum InitialEntry: CaseIterable {
        case pairing, send, signing

        var entry: BsvColdWalletEntry {
            switch self {
            case .pairing: .pairing
            case .send: .send
            case .signing: .signing
            }
        }

        var route: BsvColdWalletRoute {
            switch self {
            case .pairing: .pairing
            case .send: .send
            case .signing: .signing
            }
        }

        var scene: Scenes.BsvColdWallet.Entry {
            switch self {
            case .pairing: .pairing
            case .send: .send
            case .signing: .signing
            }
        }

        var child: BsvColdWalletRoute? {
            switch self {
            case .pairing: nil
            case .send: .request
            case .signing: .review
            }
        }
    }

    enum InitialOnboardingEntry: CaseIterable {
        case create, recover, connect

        var entry: BsvColdWalletEntry {
            switch self {
            case .create: .create
            case .recover: .recover
            case .connect: .connect
            }
        }

        var route: BsvColdWalletRoute {
            switch self {
            case .create: .create
            case .recover: .recover
            case .connect: .connect
            }
        }
    }

    enum InitialBackControl: CaseIterable {
        case home, missingCallback, mismatchedInitial, foreignEntry, foreignChild
    }

    enum EntryExit: CaseIterable {
        case back, replace
    }

    @MainActor
    private final class SelectionFixture {
        let directory: URL
        let a: BsvHomeCore.Wallet
        let b: BsvHomeCore.Wallet
        let c: BsvHomeCore.Wallet
        let session: BsvHomeWalletSessionService
        let preferences: BsvHomePreferencesStoreMock
        let ports: ForbiddenPorts
        private let wallets: BsvHomeWalletService
        private let addresses: BsvHomeBsvAddressPool
        private let gateway: BsvHomeGateway
        private let keystore: BsvHomeKeystore
        private let store: BsvHomeCoreBsvColdWalletStore
        private var services: [ObjectIdentifier: BsvHomeBsvColdWalletService] = [:]

        private init(directory: URL, wallets: [BsvHomeCore.Wallet], session: BsvHomeWalletSessionService, preferences: BsvHomePreferencesStoreMock, ports: ForbiddenPorts, walletService: BsvHomeWalletService, addresses: BsvHomeBsvAddressPool, gateway: BsvHomeGateway, keystore: BsvHomeKeystore, store: BsvHomeCoreBsvColdWalletStore) {
            self.directory = directory
            a = wallets[0]; b = wallets[1]; c = wallets[2]
            self.session = session
            self.preferences = preferences
            self.ports = ports
            self.wallets = walletService
            self.addresses = addresses
            self.gateway = gateway
            self.keystore = keystore
            self.store = store
        }

        static func make() async throws -> SelectionFixture {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
            do {
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                let configuration: GRDB.Configuration = DB.defaultConfiguration
                let database = try DB(fileURL: directory.appendingPathComponent("selection.sqlite"), configuration: configuration)
                let assetStore = AssetStore(db: database)
                let assetConfig = BsvHomeAssetConfigService()
                let asset = assetConfig.defaultAssetBasic(asset: assetConfig.chainAsset(chain: Primitives.Chain.bitcoinSv.rawValue).asset)
                try await BsvHomeCoreAssetStore(assetStore: assetStore, balanceStore: BalanceStore(db: database)).saveAssets(assets: [asset])
                let nativeWallets = BsvHomeCoreWalletStore(store: WalletStore(db: database))
                let publicAddresses = ["1BitcoinEaterAddressDontSendf59kuE", "1BoatSLRHtKNngkdXEeobR76b53LETtpyT", "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA"]
                let wallets = publicAddresses.map { address in
                    BsvHomeCore.Wallet(id: Primitives.WalletId.view(chain: .bitcoinSv, address: address).id, externalId: nil, name: "Public selection fixture", index: 0, walletType: .view, accounts: [BsvHomeCore.Account(chain: Primitives.Chain.bitcoinSv.rawValue, address: address, derivationPath: "", extendedPublicKey: nil)], isPinned: false, imageUrl: nil, source: .import)
                }
                for wallet in wallets {
                    try await nativeWallets.addWallet(wallet: wallet)
                }
                let preferences = BsvHomePreferencesStoreMock()
                let session = BsvHomeWalletSessionService.newWithEnvironment(store: BsvHomeCoreWalletSessionStore(store: preferences), wallets: nativeWallets, environment: .wowPay)
                try session.setCurrentWalletId(walletId: wallets[0].id)
                let keystore = try BsvHomeKeystore.newWithEnvironment(baseDir: directory.appendingPathComponent("empty-keys", isDirectory: true).path, environment: .wowPay)
                let ports = ForbiddenPorts()
                let gateway = BsvHomeGateway.newWithEnvironment(provider: ports, preferences: preferences, securePreferences: BsvHomeSecureStoreMock(), environment: .wowPay)
                let addresses = BsvHomeBsvAddressPool(gateway: gateway, walletStore: nativeWallets, store: BsvHomeCoreBsvAddressStore(store: BsvAddressPoolStore(db: database)), keystore: keystore, password: ports)
                let appPreferences = BsvHomePreferencesService(store: preferences)
                let walletService = BsvHomeWalletService(keystore: keystore, password: ports, store: nativeWallets, session: session, appPreferences: appPreferences, files: BsvHomeFileStoreMock(), preferences: BsvHomeWalletPreferencesService(store: BsvHomeWalletPreferencesStoreMock()), explorer: BsvHomeExplorerService(preferences: appPreferences), addresses: BsvHomeAddressStoreMock(), localizer: BsvHomeCoreLocalizer(), bsvAddresses: addresses)
                let store = try BsvHomeCoreBsvColdWalletStore(directory: directory.appendingPathComponent("cold", isDirectory: true))
                return SelectionFixture(directory: directory, wallets: wallets, session: session, preferences: preferences, ports: ports, walletService: walletService, addresses: addresses, gateway: gateway, keystore: keystore, store: store)
            } catch {
                try? FileManager.default.removeItem(at: directory)
                throw error
            }
        }

        func context(id: UUID, walletId: String?, entry: BsvColdWalletEntry = .signing, onComplete: @escaping @MainActor () -> Void = {}) -> BsvColdWalletContext {
            let service = BsvHomeBsvColdWalletService.unified(wallet: wallets, addresses: addresses, gateway: gateway, keystore: keystore, store: store, signer: ports)
            let context = BsvColdWalletContext(id: id, model: BsvColdWalletViewModel(service: service, mode: .unified, walletId: walletId, initialPage: entry, onComplete: onComplete), authorization: BsvColdWalletPasswordAuthorization(store: BsvHomeSecureStoreMock(), isForeground: { true }))
            services[ObjectIdentifier(context)] = service
            return context
        }

        func service(in context: BsvColdWalletContext) throws -> BsvHomeBsvColdWalletService {
            try #require(services[ObjectIdentifier(context)])
        }
    }

    private final class ForbiddenPorts: AlienProvider, BsvHomeKeystorePassword, BsvHomeBsvColdWalletSigner, @unchecked Sendable {
        private let lock = NSLock()
        private var accessCount = 0
        var calls: Int { lock.withLock { accessCount } }
        private func reject() throws -> Never {
            lock.withLock { accessCount += 1 }; throw BsvHomeServiceError.Unsupported(msg: "Public selection fixture forbids signing credentials")
        }

        func request(target _: AlienTarget) async throws -> AlienResponse {
            try reject()
        }

        func getPassword(createIfMissing _: Bool) throws -> String {
            try reject()
        }

        func getWalletPassword(walletId _: String) throws -> String? {
            try reject()
        }

        func deleteWalletPassword(walletId _: String) throws {
            try reject()
        }

        func authentication() throws -> BsvHomeKeystoreAuthentication {
            try reject()
        }

        func authenticateImport() async throws {
            try reject()
        }

        func authorize(wallet _: BsvHomeCore.Wallet, scope _: BsvHomeBsvColdWalletActionScope) async throws {
            try reject()
        }

        func authenticate(wallet _: BsvHomeCore.Wallet, scope _: BsvHomeBsvColdWalletActionScope) async throws {
            try reject()
        }

        func signRequest(wallet _: BsvHomeCore.Wallet, requestJson _: String, reviewJson _: String, scope _: BsvHomeBsvColdWalletActionScope) async throws -> String {
            try reject()
        }
    }
}
