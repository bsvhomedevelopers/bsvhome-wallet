import Style
import SwiftUI

private struct WowPayCardModifier: ViewModifier {
    let padding: CGFloat
    let cornerRadius: CGFloat
    let surface: Color
    let border: Color?

    func body(content: Content) -> some View {
        content
            .padding(padding)
            .background(surface, in: RoundedRectangle(cornerRadius: cornerRadius))
            .overlay {
                if let border {
                    RoundedRectangle(cornerRadius: cornerRadius)
                        .strokeBorder(border, lineWidth: .space1)
                }
            }
            .shadow(color: Colors.WowPay.shadow, radius: Sizing.wowPay.shadowRadius, y: Sizing.wowPay.shadowOffset)
    }
}

public extension View {
    func wowPayCard(
        padding: CGFloat = Spacing.wowPay.card,
        cornerRadius: CGFloat = Sizing.wowPay.cardCorner,
        surface: Color = Colors.WowPay.surface,
        border: Color? = nil,
    ) -> some View {
        modifier(WowPayCardModifier(padding: padding, cornerRadius: cornerRadius, surface: surface, border: border))
    }
}
