import BsvColdWallet
import Components
import Primitives
import SwiftUI

@MainActor
final class ColdWalletSelectionResume {
    let scene: Scenes.BsvColdWallet
    private var claimedWalletId: String?
    private enum ScanAdmission { case pending, consumed }
    private var scanAdmission = ScanAdmission.pending

    init(scene: Scenes.BsvColdWallet) {
        self.scene = scene
    }

    func walletId(for scene: Scenes.BsvColdWallet) -> String? {
        guard self.scene == scene else { return scene.walletId }
        switch scene.entry {
        case .pairing: return scene.walletId
        case .home, .send, .receive, .create, .recover, .connect, .signing, .scanRequest: return claimedWalletId ?? scene.walletId
        }
    }

    func initialPage(for scene: Scenes.BsvColdWallet) -> BsvColdWalletEntry {
        switch scene.entry {
        case .home: .home
        case .send: .send
        case .receive: .receive
        case .create: .create
        case .recover: .recover
        case .connect: .connect
        case .signing: .signing
        case .pairing: .pairing
        case .scanRequest: .scanRequest(presentation: self.scene == scene && scanAdmission == .consumed ? .resumed : .initial)
        }
    }

    func admitted(context: BsvColdWalletContext, navigation: NavigationPathState, entryPath: NavigationPath, isActive: @escaping @MainActor (BsvColdWalletContext) -> Bool) {
        guard navigation.path == entryPath else { return }
        resumeAdmittedScan(context: context, navigation: navigation, entryPath: entryPath, isActive: isActive)
    }

    private func resumeAdmittedScan(context: BsvColdWalletContext, navigation: NavigationPathState, entryPath: NavigationPath, isActive: @escaping @MainActor (BsvColdWalletContext) -> Bool) {
        guard scene.entry == .scanRequest, context.id == scene.id, !context.model.isCancelled, isActive(context), navigation.path.count >= entryPath.count else { return }
        var prefix = navigation.path
        prefix.removeLast(prefix.count - entryPath.count)
        guard prefix == entryPath else { return }
        scanAdmission = .consumed
    }

    func observe(context: BsvColdWalletContext, navigation: NavigationPathState, entryPath: NavigationPath, isActive: @escaping @MainActor (BsvColdWalletContext) -> Bool) {
        resumeAdmittedScan(context: context, navigation: navigation, entryPath: entryPath, isActive: isActive)
        context.observeWalletSelection { [weak self, weak context, weak navigation] walletId in
            guard let self, let context, let navigation, isActive(context), navigation.path.count >= entryPath.count else { return }
            var prefix = navigation.path
            prefix.removeLast(prefix.count - entryPath.count)
            guard prefix == entryPath else { return }
            switch scene.entry {
            case .pairing: return
            case .home, .send, .receive, .create, .recover, .connect, .signing, .scanRequest: claimedWalletId = walletId
            }
        }
    }
}
