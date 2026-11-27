import Style
import SwiftUI

public extension View {
    func wowPayInput(surface: Color = Colors.WowPay.surface) -> some View {
        modifier(WowPayInputModifier(surface: surface))
    }
}

private struct WowPayInputModifier: ViewModifier {
    @ScaledMetric(relativeTo: .subheadline) private var fontSize = WowPayTypography.subtitleSize
    let surface: Color

    func body(content: Content) -> some View {
        content.font(.system(size: fontSize))
            .foregroundStyle(Colors.WowPay.heading)
            .padding(.horizontal, Spacing.space12)
            .padding(.vertical, Spacing.space10)
            .background(surface, in: RoundedRectangle(cornerRadius: Sizing.wowPay.inputCorner))
            .overlay {
                RoundedRectangle(cornerRadius: Sizing.wowPay.inputCorner)
                    .strokeBorder(Colors.WowPay.inputBorder, lineWidth: .space1)
            }
    }
}
