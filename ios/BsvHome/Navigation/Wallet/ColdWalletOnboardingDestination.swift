import BsvColdWallet
import Components
import Localization
import SwiftUI

struct ColdWalletOnboardingDestination: View {
    @Environment(\.viewModelFactory) private var factory
    @Environment(RootSceneViewModel.self) private var rootModel
    @State private var id = UUID()
    @State private var context: BsvColdWalletContext?
    @State private var failed = false
    @State private var entryPath: NavigationPath

    let entry: BsvColdWalletEntry
    let owner: ObjectIdentifier
    let navigation: NavigationPathState
    let onComplete: @MainActor () -> Void

    init(entry: BsvColdWalletEntry, owner: ObjectIdentifier, navigation: NavigationPathState, onComplete: @escaping @MainActor () -> Void) {
        self.entry = entry
        self.owner = owner
        self.navigation = navigation
        self.onComplete = onComplete
        _entryPath = State(initialValue: navigation.path)
    }

    var body: some View {
        Group {
            if let context {
                BsvColdWalletScene(model: context.model, navigation: navigation, onInitialPageBack: context.onInitialPageBack)
                    .id(ObjectIdentifier(context))
                    .bsvColdWalletPassword(authorization: context.authorization)
            } else if failed {
                ContentUnavailableView {
                    Label(Localized.coldWalletTitle, systemImage: "exclamationmark.triangle")
                } description: {
                    Text(Localized.coldWalletStartupStorageError)
                } actions: {
                    Button(Localized.Common.tryAgain, action: load)
                }
            } else {
                ProgressView()
            }
        }
        .task {
            if context == nil {
                load()
            }
        }
    }

    private func installInitialPageBack(for expected: BsvColdWalletContext, entryPath: NavigationPath) {
        guard !entryPath.isEmpty else { return }
        var parent = entryPath
        parent.removeLast()
        let parentPath = parent
        expected.onInitialPageBack = { [weak rootModel, weak navigation, weak expected] in
            guard let rootModel, let navigation, let expected,
                  rootModel.activeColdWalletContext === expected, navigation.path == entryPath
            else { return }
            expected.recipientScan = nil
            rootModel.cancelColdWalletContextForNavigationRemoval(id: expected.id)
            navigation.binding.wrappedValue = parentPath
        }
    }

    private func load() {
        guard !Task.isCancelled, navigation.path.count >= entryPath.count else { return }
        var prefix = navigation.path
        prefix.removeLast(prefix.count - entryPath.count)
        guard prefix == entryPath else { return }
        do {
            let context = try factory.bsvColdWalletContext(id: id, entry: entry, onboardingOwner: owner, onComplete: onComplete)
            rootModel.registerColdWalletContext(context, navigation: navigation)
            installInitialPageBack(for: context, entryPath: entryPath)
            self.context = context
            failed = false
        } catch {
            failed = true
        }
    }
}
