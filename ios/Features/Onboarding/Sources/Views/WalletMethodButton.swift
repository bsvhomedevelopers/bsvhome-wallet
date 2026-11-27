import Localization
import Style
import SwiftUI

struct WalletMethodButton: View {
    let title: String
    let subtitle: String
    let image: Image
    let action: (() -> Void)?

    var body: some View {
        Button {
            action?()
        } label: {
            content
        }
        .buttonStyle(.plain)
        .disabled(action == nil)
        .accessibilityElement(children: .combine)
    }

    private var content: some View {
        HStack(spacing: Spacing.space16) {
            image
                .renderingMode(.template)
                .resizable()
                .scaledToFit()
                .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                .foregroundStyle(Colors.WowPay.brand)
                .frame(width: Sizing.wowPay.minimumTapTarget, height: Sizing.wowPay.minimumTapTarget)
                .background(Colors.WowPay.secondaryButton, in: RoundedRectangle(cornerRadius: Spacing.space12))
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: .zero) {
                Text(title)
                    .font(.wowPay.rowTitle.weight(.semibold))
                    .foregroundStyle(Colors.WowPay.heading)

                Text(subtitle)
                    .font(.wowPay.subtitle)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)

                if action == nil {
                    Text(Localized.wowpayUnavailable)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                        .padding(.top, Spacing.space4)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            if action != nil {
                Image("wowpay-chevron-right")
                    .renderingMode(.template)
                    .resizable()
                    .scaledToFit()
                    .frame(width: Sizing.wowPay.chevron, height: Sizing.wowPay.chevron)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .accessibilityHidden(true)
            }
        }
        .padding(.horizontal, Spacing.space16)
        .padding(.vertical, Spacing.wowPay.card)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Colors.WowPay.surface, in: RoundedRectangle(cornerRadius: Sizing.wowPay.controlCorner))
        .contentShape(RoundedRectangle(cornerRadius: Sizing.wowPay.controlCorner))
    }
}
