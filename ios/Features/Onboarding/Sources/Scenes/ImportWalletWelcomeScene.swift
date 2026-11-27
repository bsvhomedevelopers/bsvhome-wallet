import Components
import Localization
import Style
import SwiftUI

public struct ImportWalletWelcomeScene: View {
    public enum Presentation: Equatable {
        case root
        case navigation
    }

    private let presentation: Presentation
    private let onImportWallet: () -> Void
    private let onCreateWallet: () -> Void
    private let onSkip: () -> Void

    public init(
        onImportWallet: @escaping () -> Void,
        onCreateWallet: @escaping () -> Void,
        onSkip: (() -> Void)? = nil,
        presentation: Presentation = .navigation,
    ) {
        self.onImportWallet = onImportWallet
        self.onCreateWallet = onCreateWallet
        self.onSkip = onSkip ?? onCreateWallet
        self.presentation = presentation
    }

    public var body: some View {
        GeometryReader { geometry in
            ScrollView {
                VStack(spacing: .zero) {
                    VStack(spacing: Spacing.space16) {
                        brandMark(size: Sizing.wowPay.welcomeBrandMark)

                        VStack(spacing: Spacing.space8) {
                            Text(Localized.wowpayBrand)
                                .font(.wowPay.title)
                                .foregroundStyle(Colors.WowPay.heading)

                            Text(Localized.wowpayWelcomeTagline)
                                .font(.wowPay.body.weight(.regular))
                                .foregroundStyle(Colors.WowPay.secondaryText)
                        }
                    }
                    .padding(.top, presentation == .root ? Spacing.wowPay.welcomeHeaderTop : Spacing.space16)

                    Spacer(minLength: Spacing.space32)

                    brandMark(size: Sizing.image.extraLarge)

                    Spacer(minLength: Spacing.space32)

                    VStack(spacing: Spacing.space8) {
                        Text(Localized.Wallet.Import.title)
                            .font(.wowPay.onboardingTitle)
                            .foregroundStyle(Colors.WowPay.heading)

                        Text(Localized.wowpayWelcomeImportSubtitle)
                            .font(.wowPay.subtitle)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                    }

                    HStack(alignment: .top, spacing: Spacing.space16) {
                        feature(title: Localized.wowpayFeatureSecure, image: "wowpay-check-circle")
                        feature(title: Localized.wowpayFeatureFast, image: "wowpay-lightning")
                        feature(title: Localized.wowpayFeatureControl, image: "wowpay-wallet")
                    }
                    .padding(.top, Spacing.space24)

                    Spacer(minLength: Spacing.space32)
                }
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)
                .frame(minHeight: geometry.size.height)
                .padding(.horizontal, Spacing.space16)
            }
        }
        .safeAreaInset(edge: .bottom, spacing: .zero) {
            VStack(spacing: Spacing.space8) {
                StateButton(
                    text: Localized.Wallet.Import.title,
                    type: .wowPayPrimary(),
                    action: onImportWallet,
                )
                .accessibilityIdentifier("wowPayWelcomeImportWallet")

                Button(Localized.wowpayWelcomeCreateWallet, action: onCreateWallet)
                    .font(.wowPay.sectionAction)
                    .foregroundStyle(Colors.WowPay.brand)
                    .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.minimumTapTarget)
                    .accessibilityIdentifier("wowPayWelcomeCreateWallet")
            }
            .frame(maxWidth: Sizing.wowPay.welcomeButtonMaxWidth)
            .padding(.horizontal, Spacing.space16)
            .padding(.bottom, Spacing.space16)
            .frame(maxWidth: .infinity)
            .background(Colors.WowPay.background)
        }
        .wowPayBackground(.solid)
        .overlay(alignment: .topTrailing) {
            if presentation == .root {
                skipButton
                    .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget)
                    .padding(.horizontal, Spacing.space16)
                    .padding(.top, Spacing.space8)
            }
        }
        .navigationTitle("")
        .toolbarTitleDisplayMode(.inline)
        .toolbar(presentation == .root ? .hidden : .visible, for: .navigationBar)
        .toolbar {
            if presentation == .navigation {
                ToolbarItem(placement: .topBarTrailing) {
                    skipButton
                }
            }
        }
    }

    private var skipButton: some View {
        Button(Localized.Common.skip, action: onSkip)
            .buttonStyle(.plain)
            .font(.wowPay.sectionAction)
            .foregroundStyle(Colors.WowPay.brand)
            .accessibilityIdentifier("wowPayWelcomeSkip")
    }

    private func brandMark(size: CGFloat) -> some View {
        Image("wowpay-mark")
            .resizable()
            .scaledToFit()
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }

    private func feature(title: String, image: String) -> some View {
        VStack(spacing: Spacing.space8) {
            Image(image)
                .renderingMode(.template)
                .resizable()
                .scaledToFit()
                .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                .foregroundStyle(Colors.WowPay.brand)
                .frame(width: Sizing.wowPay.assetIcon, height: Sizing.wowPay.assetIcon)
                .background(Colors.WowPay.secondaryButton, in: RoundedRectangle(cornerRadius: Sizing.wowPay.controlCorner))
                .accessibilityHidden(true)

            Text(title)
                .font(.wowPay.caption.weight(.medium))
                .foregroundStyle(Colors.WowPay.heading)
        }
        .frame(maxWidth: .infinity)
    }
}
