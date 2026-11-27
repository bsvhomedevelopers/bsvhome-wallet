import Style
import SwiftUI

private struct WowPayBackgroundModifier: ViewModifier {
    let style: WowPayBackgroundStyle

    func body(content: Content) -> some View {
        content
            .background {
                switch style {
                case .gradient:
                    WowPayGradients.page.ignoresSafeArea()
                case .solid:
                    Colors.WowPay.background.ignoresSafeArea()
                }
            }
            .tint(Colors.WowPay.brand)
            .foregroundStyle(Colors.WowPay.heading)
    }
}

public extension View {
    func wowPayBackground(_ style: WowPayBackgroundStyle = .gradient) -> some View {
        modifier(WowPayBackgroundModifier(style: style))
    }
}
