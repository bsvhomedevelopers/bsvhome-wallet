import BsvColdWallet
import Components
import Foundation
import Localization
import Primitives
import QRScanner
import SwiftUI

@MainActor
struct ColdWalletDestination: View {
    @Environment(\.viewModelFactory) private var factory
    @Environment(RootSceneViewModel.self) private var rootModel
    @Environment(\.scenePhase) private var scenePhase
    @State private var context: BsvColdWalletContext?
    @State private var errorMessage: String?
    @State private var selectionResume: ColdWalletSelectionResume?
    let scene: Scenes.BsvColdWallet
    let navigation: NavigationPathState

    var body: some View {
        Group {
            if let context {
                BsvColdWalletScene(model: context.model, navigation: navigation, onScanRecipient: context.onScanRecipient, onInitialPageBack: context.onInitialPageBack)
                    .id(ObjectIdentifier(context))
            } else if let errorMessage {
                VStack {
                    ContentUnavailableView(Localized.coldWalletTitle, systemImage: "exclamationmark.triangle", description: Text(errorMessage))
                    Button(Localized.Common.tryAgain, action: load)
                }
            } else {
                ProgressView()
            }
        }
        .bsvColdWalletPassword(authorization: context?.authorization)
        .sheet(item: recipientScanBinding) { request in
            ScanQRCodeNavigationStack(scanType: .address) { value in
                guard let context, context.recipientScan?.id == request.id, request.contextId == context.id else { return }
                context.recipientScan = nil
                guard UIApplication.shared.applicationState == .active, rootModel.activeColdWalletContext === context, navigation.path == request.path else { return }
                request.completion(value)
            }
        }
        .onAppear(perform: load)
        .onChange(of: rootModel.activeColdWalletContext.map(ObjectIdentifier.init)) { _, _ in
            if rootModel.activeColdWalletContext !== context {
                context?.recipientScan = nil
                context = nil
            }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                context?.recipientScan = nil
            } else if phase == .active, context == nil {
                load()
            }
        }
    }

    private func load() {
        guard let paths = entryPaths() else { return }
        let selection = resumeSelection()
        if let active = rootModel.activeColdWalletContext, active.id == scene.id {
            selection.observe(context: active, navigation: navigation, entryPath: paths.entry) { [weak rootModel] in rootModel?.activeColdWalletContext === $0 }
            rootModel.registerColdWalletContext(active, navigation: navigation)
            installScanner(for: active, entryPath: paths.entry)
            installInitialPageBack(for: active, entryPath: paths.entry, parentPath: paths.parent)
            context = active
            errorMessage = nil
            return
        }
        guard scenePhase == .active, paths.entry == navigation.path else { return }
        if let active = rootModel.activeColdWalletContext {
            rootModel.cancelColdWalletContext(id: active.id)
        }
        do {
            weak var expected: BsvColdWalletContext?
            let rootModel = rootModel
            let navigation = navigation
            let created = try factory.bsvColdWalletContext(id: scene.id, walletId: selection.walletId(for: scene), entry: selection.initialPage(for: scene), onComplete: { [weak rootModel, weak navigation] in
                guard let rootModel, let navigation, let expected, rootModel.activeColdWalletContext === expected, navigation.path.count >= paths.entry.count else { return }
                var prefix = navigation.path
                prefix.removeLast(prefix.count - paths.entry.count)
                guard prefix == paths.entry else { return }
                navigation.binding.wrappedValue = paths.parent
            })
            expected = created
            rootModel.registerColdWalletContext(created, navigation: navigation)
            guard rootModel.activeColdWalletContext === created else { created.cancelForNavigationRemoval(); return }
            selection.admitted(context: created, navigation: navigation, entryPath: paths.entry) { [weak rootModel] in rootModel?.activeColdWalletContext === $0 }
            selection.observe(context: created, navigation: navigation, entryPath: paths.entry) { [weak rootModel] in rootModel?.activeColdWalletContext === $0 }
            installScanner(for: created, entryPath: paths.entry)
            installInitialPageBack(for: created, entryPath: paths.entry, parentPath: paths.parent)
            context = created
            errorMessage = nil
        } catch {
            errorMessage = Localized.coldWalletStartupStorageError
        }
    }

    private func installInitialPageBack(for expected: BsvColdWalletContext, entryPath: NavigationPath, parentPath: NavigationPath) {
        expected.onInitialPageBack = { [weak rootModel, weak navigation, weak expected] in
            guard let rootModel, let navigation, let expected,
                  rootModel.activeColdWalletContext === expected, navigation.path == entryPath
            else { return }
            expected.recipientScan = nil
            rootModel.cancelColdWalletContextForNavigationRemoval(id: expected.id)
            navigation.binding.wrappedValue = parentPath
        }
    }

    private func resumeSelection() -> ColdWalletSelectionResume {
        if let selectionResume, selectionResume.scene == scene {
            return selectionResume
        }
        let created = ColdWalletSelectionResume(scene: scene)
        selectionResume = created
        return created
    }

    private func entryPaths() -> (entry: NavigationPath, parent: NavigationPath)? {
        var prefix = navigation.path
        while !prefix.isEmpty {
            let entry = prefix
            prefix.removeLast()
            var expected = prefix
            expected.append(scene)
            if expected == entry {
                return (entry, prefix)
            }
        }
        return nil
    }

    private func installScanner(for expected: BsvColdWalletContext, entryPath: NavigationPath) {
        let contextId = expected.id
        expected.onScanRecipient = { [weak expected, weak rootModel, weak navigation] completion in
            guard let expected, let rootModel, let navigation,
                  UIApplication.shared.applicationState == .active, !expected.model.isCancelled,
                  rootModel.activeColdWalletContext === expected, expected.recipientScan == nil,
                  navigation.path.count >= entryPath.count
            else { return }
            var prefix = navigation.path
            prefix.removeLast(prefix.count - entryPath.count)
            guard prefix == entryPath else { return }
            expected.recipientScan = .init(contextId: contextId, path: navigation.path, completion: completion)
        }
    }

    private var recipientScanBinding: Binding<BsvColdWalletContext.RecipientScan?> {
        let expected = context
        let id = expected?.recipientScan?.id
        return Binding(get: {
            guard context === expected else { return nil }
            return expected?.recipientScan
        }, set: { replacement in
            guard context === expected, expected?.recipientScan?.id == id else { return }
            expected?.recipientScan = replacement
        })
    }
}
