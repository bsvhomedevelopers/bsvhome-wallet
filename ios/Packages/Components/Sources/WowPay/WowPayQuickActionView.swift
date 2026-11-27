import Style
import SwiftUI

public struct WowPayQuickActionView: View {
    @Environment(\.isEnabled) private var isEnabled

    private let title: String
    private let image: Image
    private let action: () -> Void

    public init(title: String, image: Image, action: @escaping () -> Void) {
        self.title = title
        self.image = image
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            VStack(spacing: .small) {
                RoundedRectangle(cornerRadius: Sizing.wowPay.controlCorner)
                    .fill(WowPayGradients.action)
                    .aspectRatio(Sizing.wowPay.actionAspectRatio, contentMode: .fit)
                    .overlay {
                        image
                            .renderingMode(.template)
                            .resizable()
                            .scaledToFit()
                            .frame(width: Sizing.wowPay.actionIcon, height: Sizing.wowPay.actionIcon)
                            .foregroundStyle(Colors.WowPay.onBrand)
                            .accessibilityHidden(true)
                    }
                Text(title)
                    .font(.wowPay.label)
                    .foregroundStyle(Colors.WowPay.heading)
                    .multilineTextAlignment(.center)
            }
        }
        .buttonStyle(.plain)
        .opacity(isEnabled ? 1 : WowPayOpacity.disabled)
        .accessibilityLabel(title)
    }
}

#Preview {
    HStack(spacing: Spacing.wowPay.grid) {
        WowPayQuickActionView(title: "Scan", image: Image(systemName: "qrcode.viewfinder"), action: {})
        WowPayQuickActionView(title: "Transfer", image: Image(systemName: "arrow.up.right"), action: {})
        WowPayQuickActionView(title: "Receive", image: Image(systemName: "qrcode"), action: {})
        WowPayQuickActionView(title: "Swap", image: Image(systemName: "arrow.left.arrow.right"), action: {})
    }
    .padding(Spacing.wowPay.page)
    .wowPayBackground()
}
