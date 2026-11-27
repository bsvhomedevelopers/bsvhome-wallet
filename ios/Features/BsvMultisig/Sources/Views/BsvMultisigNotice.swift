import Style
import SwiftUI

struct BsvMultisigNotice: View {
    let text: String
    var tone: BsvMultisigNoticeTone = .information

    var body: some View {
        Text(text)
            .font(.caption)
            .lineSpacing(Spacing.space4)
            .foregroundStyle(tone.color)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Spacing.space16)
            .background(tone.background, in: RoundedRectangle(cornerRadius: Sizing.wowPay.pairCorner))
    }
}
