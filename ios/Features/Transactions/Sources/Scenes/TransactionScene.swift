// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import PrimitivesComponents
import Style
import SwiftUI

public struct TransactionScene: View {
    private let model: TransactionSceneViewModel

    public init(model: TransactionSceneViewModel) {
        self.model = model
    }

    public var body: some View {
        ListSectionView(
            provider: model,
            content: { item in
                content(for: item)
                    .listRowInsets(EdgeInsets(
                        top: Spacing.wowPay.assetRowVertical,
                        leading: Spacing.wowPay.card,
                        bottom: Spacing.wowPay.assetRowVertical,
                        trailing: Spacing.wowPay.card,
                    ))
                    .listRowSeparatorTint(Colors.WowPay.border)
                    .listRowBackground(Colors.WowPay.surface)
            },
        )
        .listStyle(.insetGrouped)
        .contentMargins(.horizontal, Spacing.wowPay.page, for: .scrollContent)
        .contentMargins(.top, Spacing.wowPay.card, for: .scrollContent)
        .listSectionSpacing(.custom(Spacing.wowPay.grid))
        .scrollContentBackground(.hidden)
        .wowPayBackground()
        .navigationTitle(model.title)
        .navigationBarTitleDisplayMode(.inline)
    }

    @ViewBuilder
    private func content(for itemModel: TransactionItemModel) -> some View {
        switch itemModel {
        case let .listItem(model):
            ListItemView(model: model, style: .wowPay)
        case let .fee(model):
            NavigationCustomLink(
                with: ListItemView(model: model, style: .wowPay),
                action: self.model.onSelectFeeDetails,
            )
        case let .header(model):
            TransactionHeaderListItemView(
                model: model,
                style: .wowPay,
                action: self.model.onTransactionHeaderTap,
            )
        case let .swapProgress(model):
            TransactionSwapProgressView(model: model)
        case let .participant(model):
            AddressListItemView(model: model.addressViewModel, style: .wowPay)
        case let .rate(title, value):
            ListItemRotateView(
                title: title,
                subtitle: value,
                style: .wowPay,
                action: model.switchRateDirection,
            )
        case let .network(title, subtitle, image):
            ListItemImageView(
                title: title,
                subtitle: subtitle,
                assetImage: image,
                style: .wowPay,
            )
        case let .pnl(title, value, color):
            ListItemView(
                title: title,
                titleStyle: .wowPay.rowTitle,
                subtitle: value,
                subtitleStyle: TextStyle(font: .wowPay.subtitle, color: color),
            )
        case let .price(title, value):
            ListItemView(
                title: title,
                titleStyle: .wowPay.rowTitle,
                subtitle: value,
                subtitleStyle: .wowPay.value,
            )
        case let .explorer(url, text):
            SafariNavigationLink(url: url) {
                Text(text)
                    .font(.wowPay.rowTitle)
                    .foregroundStyle(Colors.WowPay.brand)
            }
        case let .swapAgain(text):
            let button = StateButton(
                text: text,
                type: .wowPayPrimary(.normal),
                action: model.onSelectSwapAgain,
            )
            .cleanListRow(topOffset: .zero)
            if #available(iOS 26, *) {
                button
            }
        case .empty:
            EmptyView()
        }
    }
}
