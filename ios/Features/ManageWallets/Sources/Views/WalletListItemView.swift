// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import enum BsvHomeCore.BsvHomeBsvColdWalletRole
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

struct WalletListItemView: View {
    let model: WalletViewModel
    let currentWalletId: WalletId?
    private let coldRole: BsvHomeBsvColdWalletRole?

    let onSelect: (Wallet) -> Void
    let onEdit: (Wallet) -> Void
    let onPin: (Wallet) -> Void
    let onDelete: (Wallet) -> Void

    init(
        wallet: Wallet,
        currentWalletId: WalletId?,
        coldRole: BsvHomeBsvColdWalletRole? = nil,
        onSelect: @escaping (Wallet) -> Void,
        onEdit: @escaping (Wallet) -> Void,
        onPin: @escaping (Wallet) -> Void,
        onDelete: @escaping (Wallet) -> Void,
    ) {
        self.currentWalletId = currentWalletId
        self.coldRole = coldRole
        model = WalletViewModel(wallet: wallet)
        self.onSelect = onSelect
        self.onEdit = onEdit
        self.onPin = onPin
        self.onDelete = onDelete
    }

    private var isSelected: Bool {
        currentWalletId == model.wallet.id
    }

    private var roleTitle: String? {
        if coldRole == .signer {
            return Localized.coldWalletTitle
        }
        if model.wallet.type == .view {
            return Localized.Info.WatchWallet.title
        }
        return nil
    }

    var body: some View {
        HStack(spacing: Spacing.space12) {
            Button(action: { onSelect(model.wallet) }) {
                HStack(spacing: Spacing.space12) {
                    ListItemView(
                        title: model.name,
                        titleStyle: .wowPay.rowTitle,
                        titleTag: roleTitle,
                        titleTagStyle: .wowPay.caption,
                        titleExtra: model.subType,
                        titleStyleExtra: .wowPay.caption,
                        imageStyle: .asset(assetImage: model.avatarImage),
                    )
                    .frame(maxWidth: .infinity, alignment: .leading)

                    if isSelected {
                        SelectionImageView()
                            .accessibilityHidden(true)
                    }
                }
                .frame(minHeight: Sizing.wowPay.minimumTapTarget)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(isSelected ? [.isSelected] : [])

            Button(action: { onEdit(model.wallet) }) {
                Images.System.settings
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .frame(width: Sizing.wowPay.minimumTapTarget, height: Sizing.wowPay.minimumTapTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Localized.Settings.title)
            .accessibilityValue(model.name)
        }
        .wowPayListCard(isSelected: isSelected)
        .contextMenu(
            [
                .custom(
                    title: Localized.Settings.title,
                    systemImage: SystemImage.settings,
                    action: { onEdit(model.wallet) },
                ),
                .pin(
                    isPinned: model.wallet.isPinned,
                    onPin: { onPin(model.wallet) },
                ),
                .delete { onDelete(model.wallet) },
            ],
        )
        .swipeActions {
            Button(
                action: { onEdit(model.wallet) },
                label: {
                    Label("", systemImage: SystemImage.settings)
                },
            )
            .tint(Colors.gray)
            Button(
                Localized.Common.delete,
                action: { onDelete(model.wallet) },
            )
            .tint(Colors.red)
        }
    }
}
