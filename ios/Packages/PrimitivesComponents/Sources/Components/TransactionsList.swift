// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Primitives
import Style
import SwiftUI

public struct TransactionsList: View {
    private let sections: [ListSection<TransactionViewModel>]
    private let currency: String
    private let style: ListItemView.Style

    public init(sections: [ListSection<TransactionViewModel>], currency: String, style: ListItemView.Style = .standard) {
        self.sections = sections
        self.currency = currency
        self.style = style
    }

    public var body: some View {
        ForEach(sections) { section in
            Section {
                ForEach(section.values) { model in
                    NavigationLink(value: Scenes.Transaction(transaction: model.transaction)) {
                        TransactionView(model: model, currency: currency, style: style)
                    }
                    .listRowInsets(style == .wowPay ? EdgeInsets(
                        top: Spacing.wowPay.transactionRowPadding,
                        leading: Spacing.wowPay.transactionRowPadding,
                        bottom: Spacing.wowPay.transactionRowPadding,
                        trailing: Spacing.wowPay.transactionRowPadding,
                    ) : .assetListRowInsets)
                    .listRowBackground(style == .wowPay ? Colors.WowPay.surface : nil)
                    .listRowSeparatorTint(style == .wowPay ? Colors.WowPay.border : nil)
                }
            } header: {
                section.title.map { title in
                    Text(title)
                        .font(style == .wowPay ? .wowPay.label : nil)
                        .foregroundStyle(style == .wowPay ? AnyShapeStyle(Colors.WowPay.secondaryText) : AnyShapeStyle(.foreground))
                }
            }
        }
    }
}
