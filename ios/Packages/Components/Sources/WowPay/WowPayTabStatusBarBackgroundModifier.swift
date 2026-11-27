import Style
import SwiftUI

private struct WowPayTabStatusBarBackgroundModifier: ViewModifier {
    func body(content: Content) -> some View {
        content
            .overlay {
                GeometryReader { geometry in
                    Colors.WowPay.background
                        .frame(height: geometry.safeAreaInsets.top)
                        .offset(y: -geometry.safeAreaInsets.top)
                }
                .allowsHitTesting(false)
                .accessibilityHidden(true)
            }
    }
}

public extension View {
    func wowPayTabStatusBarBackground() -> some View {
        modifier(WowPayTabStatusBarBackgroundModifier())
    }
}
