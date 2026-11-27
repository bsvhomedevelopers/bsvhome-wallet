// Copyright (c). Gem Wallet. All rights reserved.

import Style
import SwiftUI

public struct ListItemRotateView: View {
    private let style: ListItemView.Style
    public let title: String?
    public let subtitle: String?
    public let action: () -> Void

    public init(title: String?, subtitle: String?, style: ListItemView.Style = .standard, action: @escaping (() -> Void)) {
        self.style = style
        self.title = title
        self.subtitle = subtitle
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            HStack {
                ListItemImageView(
                    title: title,
                    subtitle: subtitle,
                    style: style,
                )

                Images.Actions.swap
                    .resizable()
                    .renderingMode(.template)
                    .frame(size: .list.accessory)
                    .foregroundStyle(style == .wowPay ? Colors.WowPay.brand : Colors.gray)
            }
        }
    }
}
