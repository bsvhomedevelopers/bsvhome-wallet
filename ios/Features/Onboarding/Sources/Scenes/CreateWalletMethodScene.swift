import Components
import Localization
import Style
import SwiftUI

public struct CreateWalletMethodScene: View {
    private let onCreateOnline: () -> Void
    private let onCreateCold: (() -> Void)?
    private let onCreateMultisig: (() -> Void)?

    public init(
        onCreateOnline: @escaping () -> Void,
        onCreateCold: (() -> Void)? = nil,
        onCreateMultisig: (() -> Void)? = nil,
    ) {
        self.onCreateOnline = onCreateOnline
        self.onCreateCold = onCreateCold
        self.onCreateMultisig = onCreateMultisig
    }

    public var body: some View {
        ScrollView {
            VStack(spacing: Spacing.space16) {
                WalletMethodButton(
                    title: Localized.Wallet.createOnline,
                    subtitle: Localized.Wallet.createOnlineHint,
                    image: Image("wowpay-wallet"),
                    action: onCreateOnline,
                )
                .accessibilityIdentifier("wowPayCreateMethodOnline")

                WalletMethodButton(
                    title: Localized.coldWalletCreate,
                    subtitle: Localized.coldWalletDescription,
                    image: Images.System.hide,
                    action: onCreateCold,
                )
                .accessibilityIdentifier("wowPayCreateMethodCold")

                WalletMethodButton(
                    title: Localized.multisigCreate,
                    subtitle: Localized.wowpayMultisigMethodSubtitle,
                    image: Image("wowpay-wallet"),
                    action: onCreateMultisig,
                )
                .accessibilityIdentifier("wowPayCreateMethodMultisig")
            }
            .padding(Spacing.space16)
        }
        .wowPayBackground(.solid)
        .navigationTitle(Localized.Wallet.New.title)
        .toolbarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
    }
}
