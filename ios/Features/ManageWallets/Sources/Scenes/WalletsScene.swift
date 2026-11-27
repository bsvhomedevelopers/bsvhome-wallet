// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI

public struct WalletsScene: View {
    @Environment(\.dismiss) private var dismiss

    @State private var model: WalletsSceneViewModel
    private let onSelect: (() -> Void)?

    public init(model: WalletsSceneViewModel, onSelect: (() -> Void)? = nil) {
        _model = State(initialValue: model)
        self.onSelect = onSelect
    }

    public var body: some View {
        List {
            Section {
                Button(action: model.onSelectCreateWallet) {
                    ListItemView(
                        title: Localized.Wallet.createNewWallet,
                        titleStyle: .wowPay.rowTitle,
                        imageStyle: .list(assetImage: .image(Images.Wallets.create)),
                    )
                    .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.minimumTapTarget, alignment: .leading)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .wowPayListCard()
                .accessibilityIdentifier("createLocalWallet")

                Button(action: model.onSelectImportWallet) {
                    ListItemView(
                        title: Localized.Wallet.importExistingWallet,
                        titleStyle: .wowPay.rowTitle,
                        imageStyle: .list(assetImage: .image(Images.Wallets.import)),
                    )
                    .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.minimumTapTarget, alignment: .leading)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .wowPayListCard()
                .accessibilityIdentifier("importLocalWallet")
            }

            if !model.personalPinnedWallets.isEmpty {
                Section {
                    ForEach(model.personalPinnedWallets) {
                        WalletListItemView(
                            wallet: $0,
                            currentWalletId: model.currentWalletId,
                            coldRole: model.coldRole(for: $0),
                            onSelect: onSelectWallet,
                            onEdit: model.onEdit,
                            onPin: { wallet in Task { await model.onPin(wallet: wallet) } },
                            onDelete: model.onDelete,
                        )
                    }
                } header: {
                    HStack {
                        Images.System.pin
                        Text(Localized.Common.pinned)
                    }
                    .font(.wowPay.label)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .textCase(nil)
                }
            }

            Section {
                ForEach(model.personalWallets) {
                    WalletListItemView(
                        wallet: $0,
                        currentWalletId: model.currentWalletId,
                        coldRole: model.coldRole(for: $0),
                        onSelect: onSelectWallet,
                        onEdit: model.onEdit,
                        onPin: { wallet in Task { await model.onPin(wallet: wallet) } },
                        onDelete: model.onDelete,
                    )
                }
            }
            if !model.coldSignerWallets.isEmpty {
                Section(Localized.coldWalletTitle) {
                    ForEach(model.coldSignerWallets) { wallet in
                        WalletListItemView(
                            wallet: wallet,
                            currentWalletId: model.currentWalletId,
                            coldRole: model.coldRole(for: wallet),
                            onSelect: onSelectWallet,
                            onEdit: model.onEdit,
                            onPin: { wallet in Task { await model.onPin(wallet: wallet) } },
                            onDelete: model.onDelete,
                        )
                    }
                }
            }
            if let error = model.coldRoleError {
                Section {
                    Text(error).foregroundStyle(Colors.WowPay.secondaryText)
                    Button(Localized.Common.tryAgain) { model.refreshColdRoles() }
                        .accessibilityIdentifier("walletColdRoleRetry")
                }
            } else if !model.coldRolesReady {
                Section { ProgressView().frame(maxWidth: .infinity) }
            }
        }
        .listStyle(.plain)
        .contentMargins(.horizontal, Spacing.wowPay.page, for: .scrollContent)
        .contentMargins(.top, Spacing.wowPay.card, for: .scrollContent)
        .scrollContentBackground(.hidden)
        .wowPayBackground()
        .tint(Colors.WowPay.brand)
        .alertSheet($model.isPresentingAlertMessage)
        .alert(
            Localized.Common.deleteConfirmation(model.walletDelete?.name ?? ""),
            presenting: $model.walletDelete,
            sensoryFeedback: .warning,
            actions: { wallet in
                Button(
                    Localized.Common.delete,
                    role: .destructive,
                    action: { Task { await model.onDeleteConfirmed(wallet: wallet) } },
                )
            },
            message: {
                if let warning = model.walletDeleteWarning {
                    Text(warning)
                }
            },
        )
        .navigationTitle(model.title)
        .navigationBarTitleDisplayMode(.inline)
        .bindQuery(model.pinnedWalletsQuery, model.walletsQuery)
        .task(id: model.coldRoleInput) { model.refreshColdRoles() }
        .onChange(of: model.hasWallets) {
            model.onChangeWallets(dismiss: dismiss)
        }
    }

    private func onSelectWallet(_ wallet: Wallet) {
        model.onSelect(wallet: wallet) {
            if let onSelect {
                onSelect()
            } else {
                dismiss()
            }
        }
    }
}
