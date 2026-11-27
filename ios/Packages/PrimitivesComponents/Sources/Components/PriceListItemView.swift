// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import Style
import SwiftUI

public struct PriceListItemView: View {
    private let style: ListItemView.Style
    let model: PriceListItemViewModel

    public init(model: PriceListItemViewModel, style: ListItemView.Style = .standard) {
        self.style = style
        self.model = model
    }

    public var body: some View {
        HStack {
            ListItemView(title: model.title, titleStyle: style == .standard ? .body : .wowPay.rowTitle)

            if model.showAmount {
                Spacer()

                HStack(spacing: .tiny) {
                    Text(model.priceAmount.text)
                        .textStyle(style == .standard ? model.priceAmount.style : model.priceAmount.style.wowPay(font: .wowPay.subtitle, color: Colors.WowPay.heading))
                    Text(model.priceChangeView.text)
                        .textStyle(model.priceChangeView.style)
                }
                .numericTransition(for: [model.priceAmount.text, model.priceChangeView.text])
            }
        }
    }
}
