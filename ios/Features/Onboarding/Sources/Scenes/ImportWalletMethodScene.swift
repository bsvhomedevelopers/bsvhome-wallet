import Components
import Localization
import Style
import SwiftUI

public struct ImportWalletMethodScene: View {
    private let onPhrase: () -> Void
    private let onPrivateKey: () -> Void
    private let onWatch: () -> Void
    private let onColdWallet: (() -> Void)?
    private let onConnectColdWallet: (() -> Void)?
    private let onJoinMultisig: (() -> Void)?

    public init(
        onPhrase: @escaping () -> Void,
        onPrivateKey: @escaping () -> Void,
        onWatch: @escaping () -> Void,
        onColdWallet: (() -> Void)? = nil,
        onConnectColdWallet: (() -> Void)? = nil,
        onJoinMultisig: (() -> Void)? = nil,
    ) {
        self.onPhrase = onPhrase
        self.onPrivateKey = onPrivateKey
        self.onWatch = onWatch
        self.onColdWallet = onColdWallet
        self.onConnectColdWallet = onConnectColdWallet
        self.onJoinMultisig = onJoinMultisig
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: .zero) {
                Text(Localized.Wallet.Import.title)
                    .font(.wowPay.title)
                    .foregroundStyle(Colors.WowPay.heading)

                Text(Localized.wowpayImportMethods)
                    .font(.wowPay.body)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .padding(.top, Spacing.space16)

                VStack(spacing: Spacing.space16) {
                    WalletMethodButton(
                        title: Localized.Common.secretPhrase,
                        subtitle: Localized.wowpayImportMethodPhraseSubtitle,
                        image: Images.System.textPageFill,
                        action: onPhrase,
                    )
                    .accessibilityIdentifier("wowPayImportMethodPhrase")

                    WalletMethodButton(
                        title: Localized.Common.privateKey,
                        subtitle: Localized.wowpayImportMethodPrivateKeySubtitle,
                        image: Images.System.hide,
                        action: onPrivateKey,
                    )
                    .accessibilityIdentifier("wowPayImportMethodPrivateKey")

                    WalletMethodButton(
                        title: Localized.wowpayKeystore,
                        subtitle: Localized.wowpayImportMethodKeystoreSubtitle,
                        image: Images.System.textPageFill,
                        action: nil,
                    )
                    .accessibilityIdentifier("wowPayImportMethodKeystore")

                    WalletMethodButton(
                        title: Localized.Info.WatchWallet.title,
                        subtitle: Localized.wowpayImportMethodWatchSubtitle,
                        image: Images.System.eye,
                        action: onWatch,
                    )
                    .accessibilityIdentifier("wowPayImportMethodWatch")

                    WalletMethodButton(
                        title: Localized.coldWalletTitle,
                        subtitle: Localized.coldWalletRecover,
                        image: Images.System.qrCodeViewfinder,
                        action: onColdWallet,
                    )
                    .accessibilityIdentifier("wowPayImportMethodColdWallet")

                    WalletMethodButton(
                        title: Localized.coldWalletConnect,
                        subtitle: Localized.wowpayColdObserverHint,
                        image: Images.System.qrCodeViewfinder,
                        action: onConnectColdWallet,
                    )
                    .accessibilityIdentifier("wowPayImportMethodConnectColdWallet")

                    WalletMethodButton(
                        title: Localized.multisigJoin,
                        subtitle: Localized.multisigJoinNotice,
                        image: Image("wowpay-wallet"),
                        action: onJoinMultisig,
                    )
                    .accessibilityIdentifier("wowPayImportMethodMultisig")
                }
                .padding(.top, Spacing.space32 + Spacing.space8)

                Text(Localized.wowpayImportMethodSafetyNote)
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(Spacing.space16)
                    .background(Colors.WowPay.secondaryButton, in: RoundedRectangle(cornerRadius: Sizing.wowPay.controlCorner))
                    .padding(.top, Spacing.space24)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Spacing.space16)
        }
        .wowPayBackground(.solid)
        .navigationTitle("")
        .toolbarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
    }
}
