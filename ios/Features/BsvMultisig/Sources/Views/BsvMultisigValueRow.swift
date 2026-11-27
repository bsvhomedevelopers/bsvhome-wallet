import Localization
import Style
import SwiftUI

struct BsvMultisigValueRow: View {
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    let title: String
    let value: String
    var onCopy: (() -> Void)?

    var body: some View {
        VStack(spacing: .zero) {
            Group {
                if dynamicTypeSize.isAccessibilitySize {
                    VStack(alignment: .leading, spacing: Spacing.small) {
                        label
                        detail.multilineTextAlignment(.leading)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                } else {
                    HStack(alignment: .top, spacing: Spacing.wowPay.card) {
                        label
                        Spacer(minLength: .zero)
                        detail.multilineTextAlignment(.trailing)
                    }
                }
            }
            .padding(.vertical, Spacing.space12)
            Divider().overlay(Colors.WowPay.border)
        }
    }

    private var label: some View {
        Text(title).font(.footnote).foregroundStyle(Colors.WowPay.secondaryText)
    }

    private var detail: some View {
        HStack(alignment: .top, spacing: Spacing.space4) {
            Text(value).font(.wowPay.multisigValue).textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
            if let onCopy {
                Button(Localized.Common.copy, systemImage: SystemImage.copy, action: onCopy)
                    .labelStyle(.iconOnly).buttonStyle(.plain)
                    .frame(minWidth: Spacing.wowPay.minimumTapTarget, minHeight: Spacing.wowPay.minimumTapTarget)
                    .foregroundStyle(Colors.WowPay.brand)
                    .accessibilityValue(value)
            }
        }
    }
}
