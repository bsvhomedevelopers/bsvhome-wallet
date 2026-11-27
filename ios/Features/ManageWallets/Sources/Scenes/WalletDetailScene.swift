// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import Localization
import Onboarding
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI

public struct WalletDetailScene: View {
    enum Field: Int, Hashable {
        case name
    }

    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @FocusState private var focusedField: Field?
    @State private var model: WalletDetailViewModel

    public init(model: WalletDetailViewModel) {
        _model = State(initialValue: model)
    }

    public var body: some View {
        ScrollView {
            VStack(spacing: Spacing.wowPay.section) {
                walletIdentity
                secretSection
                addressSection
                coldActionsSection
                Button(role: .destructive, action: model.onSelectDelete) {
                    Text(Localized.Common.delete)
                        .font(.wowPay.rowTitle)
                        .foregroundStyle(Colors.red)
                        .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.buttonHeight)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .wowPayCard(padding: .zero, cornerRadius: Sizing.wowPay.controlCorner)
            }
            .padding(Spacing.wowPay.page)
        }
        .scrollDismissesKeyboard(.interactively)
        .wowPayBackground()
        .tint(Colors.WowPay.brand)
        .bindQuery(model.walletQuery)
        .task(id: model.wallet.id) { await model.loadSecretKind() }
        .task(id: model.walletQuery.value) { await model.loadColdPresentation() }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                model.clearSecrets()
            }
        }
        .onDisappear { model.clearSecrets() }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.protectedDataWillBecomeUnavailableNotification)) { _ in model.clearSecrets() }
        .onReceive(NotificationCenter.default.publisher(for: UIScreen.capturedDidChangeNotification)) { _ in
            if UIScreen.main.isCaptured {
                model.clearSecrets()
            }
        }
        .onChange(of: model.nameInput) { Task { await model.onChangeWalletName() } }
        .navigationTitle(model.title)
        .navigationBarTitleDisplayMode(.inline)
        .alert(
            Localized.Common.deleteConfirmation(model.name),
            presenting: $model.isPresentingDeleteConfirmation,
            sensoryFeedback: .warning,
            actions: { _ in
                Button(
                    Localized.Common.delete,
                    role: .destructive,
                    action: {
                        Task {
                            if await model.onDelete() {
                                dismiss()
                            }
                        }
                    },
                )
            },
            message: {
                if let warning = model.walletDeleteWarning {
                    Text(warning)
                }
            },
        )
        .alertSheet($model.isPresentingAlertMessage)
        .sheet(item: $model.isPresentingExportWallet) {
            ExportWalletNavigationStack(flow: $0)
        }
    }
}

extension WalletDetailScene {
    private var walletIdentity: some View {
        VStack(spacing: Spacing.wowPay.card) {
            AvatarView(
                avatarImage: model.avatarAssetImage(for: model.wallet),
                size: .image.extraLarge,
                action: model.onSelectImage,
            )
            .buttonStyle(.plain)
            .accessibilityLabel(Localized.Common.avatar)
            .accessibilityValue(model.name)

            if let role = model.coldRoleTitle {
                Text(role)
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }

            FloatTextField(
                Localized.Wallet.name,
                text: $model.nameInput,
                style: .wowPay,
                allowClean: focusedField == .name,
            )
            .focused($focusedField, equals: .name)
            .accessibilityIdentifier("walletDetailName")
            .wowPayInput()
        }
        .frame(maxWidth: .infinity)
        .wowPayCard()
    }

    @ViewBuilder
    private var secretSection: some View {
        switch model.secretKind {
        case .phrase:
            secretRow(title: Localized.Common.secretPhrase)
        case .privateKey:
            secretRow(title: Localized.Common.privateKey)
        case .recovery:
            secretRow(title: Localized.bsvHdBackupTitle)
        case nil:
            if let error = model.secretMetadataError {
                Text(error).foregroundStyle(Colors.WowPay.secondaryText)
            }
        }
    }

    private func secretRow(title: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.space12) {
            Text(title)
                .font(.wowPay.sectionTitle)
                .foregroundStyle(Colors.WowPay.heading)
            Button(action: model.onShowSecret) {
                HStack(spacing: Spacing.space12) {
                    ListItemView(title: Localized.Common.show(title), titleStyle: .wowPay.rowTitle)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Image("wowpay-chevron-right")
                        .resizable()
                        .scaledToFit()
                        .frame(width: Sizing.wowPay.chevron, height: Sizing.wowPay.chevron)
                        .accessibilityHidden(true)
                }
                .frame(minHeight: Sizing.wowPay.minimumTapTarget)
                .wowPayCard(padding: Spacing.wowPay.rowPadding, cornerRadius: Sizing.wowPay.controlCorner)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(model.isRevealingSecret)
        }
    }

    @ViewBuilder
    private var coldActionsSection: some View {
        if model.hasColdPresentation {
            if model.isLoadingColdPresentation {
                ProgressView().frame(maxWidth: .infinity)
            } else if let error = model.coldPresentationError {
                VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                    Text(error).foregroundStyle(Colors.WowPay.secondaryText)
                    Button(Localized.Common.tryAgain) { Task { await model.loadColdPresentation() } }
                        .buttonStyle(.wowPaySecondary())
                        .accessibilityIdentifier("walletDetailColdPairingRetry")
                }
            } else {
                if model.canOpenColdPairing {
                    Button(action: model.onSelectColdPairing) {
                        ListItemView(title: Localized.coldWalletPair, titleStyle: .wowPay.rowTitle, imageStyle: .list(assetImage: .image(Image(systemName: "qrcode"))))
                            .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.minimumTapTarget, alignment: .leading)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .wowPayCard(padding: Spacing.wowPay.rowPadding, cornerRadius: Sizing.wowPay.controlCorner)
                    .accessibilityIdentifier("walletDetailColdPairing")
                }
                if model.canOpenColdSigning {
                    Button(action: model.onSelectColdSigning) {
                        ListItemView(title: Localized.coldWalletSigning, titleStyle: .wowPay.rowTitle, imageStyle: .list(assetImage: .image(Image("wowpay-key"))))
                            .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.minimumTapTarget, alignment: .leading)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .wowPayCard(padding: Spacing.wowPay.rowPadding, cornerRadius: Sizing.wowPay.controlCorner)
                    .accessibilityIdentifier("walletDetailColdSigning")
                }
            }
        }
    }

    @ViewBuilder
    private var addressSection: some View {
        switch model.address {
        case let .account(account):
            AddressListItemView(
                model: AddressListItemViewModel(
                    title: Localized.Common.address,
                    account: account,
                    mode: .auto(addressStyle: .short),
                    addressLink: model.addressLink(account: account),
                ),
                style: .wowPay,
            )
            .wowPayCard(padding: Spacing.wowPay.rowPadding, cornerRadius: Sizing.wowPay.controlCorner)
        case .none:
            EmptyView()
        }
    }
}
