import Components
import Localization
import Style
import SwiftUI

public struct CreateWalletIntroScene: View {
    private let onCreateWallet: () -> Void
    private let onNotifications: () -> Void

    public init(
        onCreateWallet: @escaping () -> Void,
        onNotifications: @escaping () -> Void,
    ) {
        self.onCreateWallet = onCreateWallet
        self.onNotifications = onNotifications
    }

    public var body: some View {
        ScrollView {
            VStack(spacing: .zero) {
                VStack(spacing: Spacing.space12) {
                    Image("wowpay-shield-check")
                        .renderingMode(.template)
                        .resizable()
                        .scaledToFit()
                        .frame(width: Sizing.image.semiLarge, height: Sizing.image.semiLarge)
                        .foregroundStyle(Colors.WowPay.brand)
                        .accessibilityHidden(true)

                    Text(Localized.wowpayWelcomeCreateWallet)
                        .font(.wowPay.onboardingTitle)
                        .foregroundStyle(Colors.WowPay.heading)

                    Text(Localized.wowpayCreateSubtitle)
                        .font(.wowPay.body)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)
                .padding(.top, Spacing.wowPay.welcomeVertical + Spacing.space24)

                HStack(spacing: Spacing.space16) {
                    Image("wowpay-shield-check")
                        .renderingMode(.template)
                        .resizable()
                        .scaledToFit()
                        .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                        .foregroundStyle(Colors.WowPay.brand)
                        .accessibilityHidden(true)

                    VStack(alignment: .leading, spacing: Spacing.space6) {
                        Text(Localized.wowpayCreateOwnershipTitle)
                            .font(.wowPay.rowTitle)
                            .foregroundStyle(Colors.WowPay.heading)

                        Text(Localized.wowpayCreateOwnershipSubtitle)
                            .font(.wowPay.caption)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .wowPayCard(padding: Spacing.space16, cornerRadius: Sizing.wowPay.rowCorner, border: Colors.WowPay.border)
                .padding(.top, Spacing.space32)

                StateButton(
                    text: Localized.wowpayCreateAction,
                    type: .wowPayPrimary(),
                    action: onCreateWallet,
                )
                .accessibilityIdentifier("wowPayCreateIntroSubmit")
                .padding(.top, Spacing.space24)
            }
            .frame(maxWidth: .infinity)
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .navigationTitle(Localized.Wallet.New.title)
        .toolbarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(action: onNotifications) {
                    Image("wowpay-bell")
                        .resizable()
                        .scaledToFit()
                        .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                }
                .accessibilityLabel(Localized.Settings.Notifications.title)
                .accessibilityIdentifier("wowPayCreateIntroNotifications")
            }
        }
    }
}
