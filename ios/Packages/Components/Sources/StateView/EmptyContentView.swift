// Copyright (c). Gem Wallet. All rights reserved.

import Style
import SwiftUI

public struct EmptyContentView: View {
    public let model: any EmptyContentViewable
    private let style: ListItemView.Style

    public init(model: any EmptyContentViewable, style: ListItemView.Style = .standard) {
        self.model = model
        self.style = style
    }

    public var body: some View {
        StateEmptyView(
            title: model.title,
            titleTextStyle: style == .wowPay ? .wowPay.rowTitle : .headline,
            description: model.description,
            descriptionTextStyle: style == .wowPay ? .wowPay.value : .subHeadline,
            image: model.image,
            buttons: {
                HStack(spacing: .small) {
                    ForEach(model.buttons) { button in
                        if let action = button.action {
                            Button(button.title) {
                                action()
                            }
                            .buttonStyle(
                                style == .wowPay ? .wowPaySecondary() : .empty(),
                            )
                        }
                    }
                }
                .fixedSize(horizontal: style == .standard, vertical: true)
            },
        )
    }
}
