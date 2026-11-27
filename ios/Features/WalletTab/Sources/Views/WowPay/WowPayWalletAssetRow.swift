import Components
import Primitives
import Style
import SwiftUI

struct WowPayWalletAssetRow: View {
    let model: any ListAssetItemViewable
    let asset: Asset?
    let accessibilityIdentifier: String
    let onSelectAsset: (Asset) -> Void
    let onUnavailable: () -> Void

    var body: some View {
        Button {
            if let asset {
                onSelectAsset(asset)
            } else {
                onUnavailable()
            }
        } label: {
            content
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(accessibilityIdentifier)
    }

    private var content: some View {
        HStack(spacing: Spacing.wowPay.assetRowGap) {
            ListAssetItemView(model: model, style: .wowPay)
            Image("wowpay-chevron-right")
                .resizable().scaledToFit()
                .frame(width: Sizing.wowPay.assetChevron, height: Sizing.wowPay.assetChevron)
                .opacity(WowPayOpacity.assetChevron)
                .accessibilityHidden(true)
        }
        .padding(.vertical, Spacing.wowPay.assetRowVertical)
        .padding(.horizontal, Spacing.wowPay.assetRowHorizontal)
        .wowPayCard(padding: .zero, cornerRadius: Sizing.wowPay.rowCorner)
        .contentShape(RoundedRectangle(cornerRadius: Sizing.wowPay.rowCorner))
    }
}
