// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Style
import SwiftUI

public struct AddressListItemView: View {
    @State private var isPresentingUrl: URL? = nil
    @State private var showAddress: Bool = false
    private let style: ListItemView.Style
    private let model: AddressListItemViewModel

    public init(model: AddressListItemViewModel, style: ListItemView.Style = .standard) {
        self.style = style
        self.model = model
    }

    public var body: some View {
        VStack(alignment: .trailing, spacing: .tiny) {
            ListItemImageView(
                title: model.title,
                subtitle: model.subtitleExtra == nil ? (showAddress ? model.addressSubtitle : model.subtitle) : nil,
                assetImage: model.assetImage,
                style: style,
            )
            if let address = model.subtitleExtra {
                Text(model.subtitle)
                    .textStyle(style == .wowPay ? .wowPay.value : .calloutSecondary)
                    .multilineTextAlignment(.trailing)
                    .fixedSize(horizontal: false, vertical: true)
                Text(address)
                    .textStyle(style == .wowPay ? .wowPay.caption : .calloutSecondary)
                    .multilineTextAlignment(.trailing)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .onTap {
            if model.canToggleAddress {
                showAddress.toggle()
            }
        }
        .contextMenu(contextMenuItems)
        .safariSheet(url: $isPresentingUrl)
    }

    private var contextMenuItems: [ContextMenuItemType] {
        var items: [ContextMenuItemType] = [
            .copy(value: model.account.address),
            .url(title: model.addressExplorerText, onOpen: { isPresentingUrl = model.addressExplorerUrl }),
        ]
        if let onAddContact = model.onAddContact {
            let recipient = model.addContactRecipient
            items.append(.custom(
                title: model.createContactTitle,
                systemImage: model.createContactImage,
                action: { onAddContact(.new(recipient, chain: model.account.chain)) },
            ))
            items.append(.custom(
                title: model.addToExistingContactTitle,
                systemImage: model.addToExistingContactImage,
                action: { onAddContact(.existing(recipient, chain: model.account.chain)) },
            ))
        }
        return items
    }
}
