import Style
import SwiftUI

public struct WowPaySectionHeader: View {
    public enum Style {
        case standard
        case assetDetail
    }

    private let style: Style
    private let title: String
    private let action: (title: String, perform: () -> Void)?

    public init(title: String) {
        self.title = title
        style = .standard
        action = nil
    }

    public init(title: String, actionTitle: String, style: Style = .standard, action: @escaping () -> Void) {
        self.title = title
        self.style = style
        self.action = (actionTitle, action)
    }

    public var body: some View {
        HStack(spacing: .small) {
            Text(title)
                .font(.wowPay.sectionTitle)
                .foregroundStyle(Colors.WowPay.heading)
            Spacer(minLength: .small)
            if let action {
                HStack(spacing: .tiny) {
                    Text(action.title)
                    Image(systemName: "chevron.right")
                }
                .font(style == .assetDetail ? .wowPay.rowTitle : .wowPay.sectionAction)
                .foregroundStyle(style == .assetDetail ? Colors.WowPay.brand : Colors.WowPay.secondaryText)
                .accessibilityHidden(true)
                .overlay {
                    Button(action: action.perform) {
                        Color.clear
                            .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(action.title)
                }
            }
        }
    }
}

#Preview {
    WowPaySectionHeader(title: "Services", actionTitle: "More", action: {})
        .padding(Spacing.wowPay.page)
        .wowPayBackground()
}
