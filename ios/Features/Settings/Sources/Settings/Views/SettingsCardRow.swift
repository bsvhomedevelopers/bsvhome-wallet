// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Style
import SwiftUI

struct SettingsCardRow: View {
    let title: String
    let image: AssetImage
    var value: String?
    var chevron: Chevron = .fixedRight

    enum Chevron {
        case fixedRight, forward
    }

    var body: some View {
        HStack(spacing: Spacing.space12) {
            ListItemView(title: TextValue(text: title, style: TextStyle(font: .wowPay.rowTitle, color: Colors.WowPay.heading)), imageStyle: ListItemImageStyle(assetImage: image, imageSize: Sizing.wowPay.icon, cornerRadiusType: .none))
                .frame(maxWidth: .infinity, alignment: .leading)
            if let value {
                Text(value).font(.wowPay.subtitle).foregroundStyle(Colors.WowPay.secondaryText)
            }
            Image("wowpay-chevron-right").resizable().scaledToFit().flipsForRightToLeftLayoutDirection(chevron == .forward).frame(width: Sizing.wowPay.chevron, height: Sizing.wowPay.chevron).accessibilityHidden(true)
        }
        .wowPayCard(padding: Spacing.wowPay.rowPadding, cornerRadius: Sizing.wowPay.controlCorner).contentShape(Rectangle())
    }
}
