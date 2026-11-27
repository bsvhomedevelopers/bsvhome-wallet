import Style
import SwiftUI

struct BsvMultisigChip: View {
    let title: String
    var confirmed = false

    var body: some View {
        Text(title)
            .font(.wowPay.multisigChip)
            .foregroundStyle(confirmed ? Colors.WowPay.multisigPositiveText : Colors.WowPay.multisigWarningText)
            .padding(.vertical, Spacing.space6)
            .padding(.horizontal, Spacing.space10)
            .background(confirmed ? Colors.WowPay.multisigPositiveSurface : Colors.WowPay.multisigWarningSurface, in: Capsule())
            .fixedSize(horizontal: false, vertical: true)
    }
}
