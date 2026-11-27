import Components
import SwiftUI

private struct BsvMultisigNavigationModifier: ViewModifier {
    @State private var navigation: BsvMultisigNavigation
    let model: BsvMultisigViewModel?
    let identityContext: BsvMultisigIdentityContext?
    let onScanInvitation: BsvMultisigInvitationScanner?
    let identityContextForRoute: (@MainActor (BsvMultisigRoute) -> BsvMultisigIdentityContext?)?

    init(model: BsvMultisigViewModel?, navigationPath: Binding<NavigationPath>, identityContext: BsvMultisigIdentityContext?, onScanInvitation: BsvMultisigInvitationScanner?, identityContextForRoute: (@MainActor (BsvMultisigRoute) -> BsvMultisigIdentityContext?)?) {
        self.model = model
        self.identityContext = identityContext
        self.onScanInvitation = onScanInvitation
        self.identityContextForRoute = identityContextForRoute
        _navigation = State(initialValue: BsvMultisigNavigation(path: navigationPath))
    }

    func body(content: Content) -> some View {
        content
            .environment(navigation)
            .environment(\.bsvMultisigIdentityContext, identityContext)
            .environment(\.bsvMultisigInvitationScanner, onScanInvitation)
            .navigationDestination(for: BsvMultisigRoute.self) { route in
                if let model {
                    BsvMultisigDestination(model: model, route: route)
                        .environment(navigation)
                        .environment(\.bsvMultisigIdentityContext, context(for: route))
                        .environment(\.bsvMultisigInvitationScanner, onScanInvitation)
                } else {
                    LoadingView()
                }
            }
    }

    private func context(for route: BsvMultisigRoute) -> BsvMultisigIdentityContext? {
        if let identityContextForRoute {
            return identityContextForRoute(route)
        }
        return identityContext
    }
}

public extension View {
    func bsvMultisigNavigation(model: BsvMultisigViewModel?, navigationPath: Binding<NavigationPath>, identityContext: BsvMultisigIdentityContext? = nil, onScanInvitation: BsvMultisigInvitationScanner? = nil, identityContextForRoute: (@MainActor (BsvMultisigRoute) -> BsvMultisigIdentityContext?)? = nil) -> some View {
        modifier(BsvMultisigNavigationModifier(model: model, navigationPath: navigationPath, identityContext: identityContext, onScanInvitation: onScanInvitation, identityContextForRoute: identityContextForRoute))
    }
}
