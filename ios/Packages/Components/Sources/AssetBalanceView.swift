// Copyright (c). Gem Wallet. All rights reserved.

import Style
import SwiftUI

public struct AssetBalanceView<SecondaryView: View>: View {
    public enum Style {
        case standard
        case wowPay
    }

    private let style: Style
    let image: AssetImage
    let title: String
    let balance: String?
    let secondary: () -> SecondaryView

    public init(
        image: AssetImage,
        title: String,
        balance: String?,
        style: Style = .standard,
        @ViewBuilder secondary: @escaping () -> SecondaryView,
    ) {
        self.style = style
        self.image = image
        self.title = title
        self.balance = balance
        self.secondary = secondary
    }

    public var body: some View {
        ListItemFlexibleView(
            left: {
                AssetImageView(assetImage: image, size: style == .wowPay ? Sizing.wowPay.assetIcon : .image.asset)
            }, primary: {
                VStack(alignment: .leading, spacing: .tiny) {
                    Text(title)
                        .textStyle(style == .wowPay ? TextStyle(font: .wowPay.assetTitle, color: Colors.WowPay.heading) : .headline.weight(.semibold))
                    if let balance {
                        Text(balance)
                            .textStyle(style == .wowPay ? TextStyle(font: .wowPay.subtitle, color: Colors.WowPay.secondaryText) : TextStyle(font: .callout, color: Colors.gray, fontWeight: .medium))
                    }
                }
                .lineLimit(1)
                .truncationMode(.tail)
            },
            secondary: secondary,
        )
    }
}
