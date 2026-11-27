import Style
import SwiftUI

public struct WowPayCountBadge: View {
    @ScaledMetric(relativeTo: .caption) private var minimumSize = Sizing.wowPay.multisigBadge
    private let count: Int

    public init(count: Int) {
        self.count = count
    }

    public var body: some View {
        if count > 0 {
            Text(count > 99 ? "99+" : String(count))
                .font(.wowPay.multisigChip)
                .foregroundStyle(Colors.WowPay.onBrand)
                .padding(.horizontal, count > 9 ? Spacing.space6 : .zero)
                .frame(minWidth: minimumSize, minHeight: minimumSize)
                .background(Colors.WowPay.multisigBadge, in: Capsule())
        }
    }
}
