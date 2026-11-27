import Style
import SwiftUI

public extension View {
    func wowPayListCard(isSelected: Bool = false) -> some View {
        listRowInsets(EdgeInsets(
            top: Spacing.wowPay.assetRowVertical + Spacing.wowPay.grid / 2,
            leading: Spacing.wowPay.assetRowHorizontal,
            bottom: Spacing.wowPay.assetRowVertical + Spacing.wowPay.grid / 2,
            trailing: Spacing.wowPay.assetRowHorizontal,
        ))
        .listRowSeparator(.hidden)
        .listRowBackground(
            RoundedRectangle(cornerRadius: Sizing.wowPay.rowCorner)
                .fill(isSelected ? Colors.WowPay.selectionSurface : Colors.WowPay.surface)
                .overlay {
                    RoundedRectangle(cornerRadius: Sizing.wowPay.rowCorner)
                        .strokeBorder(isSelected ? Colors.WowPay.brand : Colors.WowPay.inputBorder, lineWidth: .space1)
                }
                .padding(.vertical, Spacing.wowPay.grid / 2),
        )
    }
}
