import Components
import Localization
import SwiftUI

private struct BsvColdWalletNavigationModifier: ViewModifier {
    let model: BsvColdWalletViewModel?
    let navigation: NavigationPathState
    let onScanRecipient: BsvColdWalletRecipientScanner?
    let onInitialPageBack: (@MainActor () -> Void)?

    func body(content: Content) -> some View {
        content.navigationDestination(for: BsvColdWalletPageRoute.self) { route in
            if let model {
                let owner = model.navigation(in: navigation)
                if owner.owns(route, model: model, navigation: navigation) {
                    BsvColdWalletScene(model: model, navigation: navigation, onScanRecipient: onScanRecipient)
                        .destination(route)
                } else {
                    ContentUnavailableView(Localized.coldWalletTitle, systemImage: "exclamationmark.triangle", description: Text(Localized.coldWalletInvalidData))
                }
            } else {
                LoadingView()
            }
        }
        .onChange(of: model?.path) { old, new in
            guard let model, !model.isCancelled, let new else { return }
            model.navigationChanged(old ?? [], new)
            model.navigation(in: navigation).show(new, model: model)
        }
        .onChange(of: navigation.path) { _, _ in
            guard let model, !model.isCancelled else { return }
            model.navigation(in: navigation).reconcile(model: model, onInitialPageBack: onInitialPageBack)
        }
        .onChange(of: model.map(ObjectIdentifier.init), initial: true) { _, _ in
            guard let model, !model.isCancelled else { return }
            model.navigation(in: navigation).appear(model: model, onInitialPageBack: onInitialPageBack)
        }
    }
}

public extension View {
    func bsvColdWalletNavigation(model: BsvColdWalletViewModel?, navigation: NavigationPathState, onScanRecipient: BsvColdWalletRecipientScanner? = nil, onInitialPageBack: (@MainActor () -> Void)? = nil) -> some View {
        modifier(BsvColdWalletNavigationModifier(model: model, navigation: navigation, onScanRecipient: onScanRecipient, onInitialPageBack: onInitialPageBack))
    }
}
