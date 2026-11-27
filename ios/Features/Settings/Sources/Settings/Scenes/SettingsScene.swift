// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Primitives
import Style
import SwiftUI

public struct SettingsScene: View {
    @State private var model: SettingsViewModel
    @Binding private var isPresentingWallets: Bool
    private let onCreateWallet: () -> Void
    private let onImportWallet: () -> Void
    private let onActivity: () -> Void
    private let onPaymail: () -> Void
    private let onAddressManagement: () -> Void
    private let onMultisig: (() -> Void)?
    private let onAccount: (() -> Void)?
    private let selectedWalletName: String?

    public init(
        model: SettingsViewModel,
        isPresentingWallets: Binding<Bool>,
        onCreateWallet: @escaping () -> Void,
        onImportWallet: @escaping () -> Void,
        onActivity: @escaping () -> Void,
        onPaymail: @escaping () -> Void,
        onAddressManagement: @escaping () -> Void,
        onMultisig: (() -> Void)? = nil,
        onAccount: (() -> Void)? = nil,
        selectedWalletName: String? = nil,
    ) {
        _model = State(initialValue: model)
        _isPresentingWallets = isPresentingWallets
        self.onCreateWallet = onCreateWallet
        self.onImportWallet = onImportWallet
        self.onActivity = onActivity
        self.onPaymail = onPaymail
        self.onAddressManagement = onAddressManagement
        self.onMultisig = onMultisig
        self.onAccount = onAccount
        self.selectedWalletName = selectedWalletName
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
                VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                    WowPayBrandHeader(
                        title: Localized.wowpayBrand,
                        subtitle: Localized.wowpayTagline,
                        mark: Image("wowpay-mark"),
                    ) {
                        EmptyView()
                    }
                    walletCard
                }
                profileSection
                settingsSection
            }
            .padding(Spacing.wowPay.page)
        }
        .buttonStyle(.plain)
        .wowPayBackground(.solid)
        .bindQuery(model.walletsQuery)
        .toolbar(.hidden, for: .navigationBar)
    }
}

extension SettingsScene {
    private var walletCard: some View {
        VStack(alignment: .leading, spacing: Spacing.medium) {
            VStack(alignment: .leading, spacing: Spacing.tiny) {
                Text(Localized.wowpayMyWallet)
                    .font(.wowPay.cardTitle)
                    .foregroundStyle(Colors.WowPay.heading)
                Text(Localized.wowpayMyWalletSub)
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            Button(action: onOpenWallets) {
                ListItemView(
                    title: selectedWalletName ?? Localized.wowpayDefaultWallet,
                    titleStyle: TextStyle(font: .wowPay.label, color: Colors.WowPay.heading),
                    subtitle: "BSV",
                    subtitleStyle: TextStyle(font: .wowPay.subtitle, color: Colors.WowPay.secondaryText),
                    imageStyle: ListItemImageStyle(
                        assetImage: .image(Image("wowpay-wallet-outline")),
                        imageSize: Sizing.wowPay.icon,
                        foregroundColor: Colors.WowPay.icon,
                        cornerRadiusType: .none,
                    ),
                )
                .padding(Spacing.wowPay.rowPadding)
                .background(Colors.WowPay.pale, in: RoundedRectangle(cornerRadius: Sizing.wowPay.controlCorner))
                .contentShape(Rectangle())
            }
            .accessibilityIdentifier("openWalletManager")
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .wowPayCard()
    }

    private var profileSection: some View {
        VStack(alignment: .leading, spacing: Spacing.space12) {
            Text(Localized.wowpayWalletManagement)
                .font(.wowPay.sectionTitle)
                .foregroundStyle(Colors.WowPay.heading)
            Button(action: onCreateWallet) {
                SettingsCardRow(title: Localized.Wallet.createNewWallet, image: .image(Image("wowpay-shield-check")))
            }
            Button(action: onImportWallet) {
                SettingsCardRow(title: Localized.Wallet.importExistingWallet, image: .image(Image("wowpay-import-wallet")))
            }
            Button(action: onPaymail) {
                SettingsCardRow(title: Localized.wowpayPaymail, image: .image(Image("wowpay-paymail")))
            }
            Button(action: onAddressManagement) {
                SettingsCardRow(title: Localized.wowpayAddressManagement, image: .image(Image("wowpay-address-book")))
            }
            if let onMultisig {
                Button(action: onMultisig) {
                    SettingsCardRow(title: Localized.multisigWalletTitle, image: .image(Image("wowpay-shield-check")))
                }
                .accessibilityIdentifier("wowPaySettingsMultisig")
            }
            Button(action: onActivity) {
                SettingsCardRow(title: Localized.Activity.title, image: .image(Image("wowpay-transaction-history")))
            }
        }
        .foregroundStyle(Colors.WowPay.icon)
    }

    private var settingsSection: some View {
        VStack(alignment: .leading, spacing: Spacing.space12) {
            Text(Localized.Settings.title)
                .font(.wowPay.sectionTitle)
                .foregroundStyle(Colors.WowPay.heading)
            if let onAccount {
                Button(action: onAccount) {
                    SettingsCardRow(title: Localized.phaseoneAccount, image: .image(Image(systemName: "person.crop.circle")))
                }
                .accessibilityIdentifier("phaseOneAccount")
            }
            Button(action: onOpenWallets) {
                SettingsCardRow(title: model.walletsTitle, image: .image(Images.Settings.wallets), value: model.walletsValue, chevron: .forward)
            }
            .accessibilityIdentifier("settingsWallets")
            NavigationLink(value: Scenes.Security()) {
                SettingsCardRow(title: Localized.Settings.security, image: .image(Image("wowpay-shield-check")), chevron: .forward)
            }
            .accessibilityIdentifier("settingsSecurity")
            NavigationLink(value: Scenes.Preferences()) {
                SettingsCardRow(title: Localized.Settings.Preferences.title, image: .image(Image("wowpay-preferences")), chevron: .forward)
            }
            .accessibilityIdentifier("settingsPreferences")
            if model.isDeveloperEnabled {
                NavigationLink(value: Scenes.Developer()) {
                    SettingsCardRow(title: Localized.Settings.developer, image: .image(Images.Settings.developer), chevron: .forward)
                }
                .accessibilityIdentifier("settingsDeveloper")
            }
        }
        .foregroundStyle(Colors.WowPay.icon)
    }
}

// MARK: - Actions

extension SettingsScene {
    private func onOpenWallets() {
        isPresentingWallets = true
    }
}
