// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Primitives
import Style
import SwiftUI

public struct ValueHeaderViewSpacing: Sendable {
    public static let standard = ValueHeaderViewSpacing(
        assetImageBottom: .space12,
        titleBottom: .space10,
        subtitleTop: .zero,
        subtitleBottom: .space10,
    )
    public static let transactionAmount = ValueHeaderViewSpacing(
        assetImageBottom: .space8,
        titleBottom: .space6,
        subtitleTop: .space2,
        subtitleBottom: .zero,
    )

    let assetImageBottom: CGFloat
    let titleBottom: CGFloat
    let subtitleTop: CGFloat
    let subtitleBottom: CGFloat

    public init(
        assetImageBottom: CGFloat,
        titleBottom: CGFloat,
        subtitleTop: CGFloat = .zero,
        subtitleBottom: CGFloat,
    ) {
        self.assetImageBottom = assetImageBottom
        self.titleBottom = titleBottom
        self.subtitleTop = subtitleTop
        self.subtitleBottom = subtitleBottom
    }

    public init(
        contentBottom: CGFloat,
        subtitleTop: CGFloat = .zero,
        subtitleBottom: CGFloat,
    ) {
        self.init(
            assetImageBottom: contentBottom,
            titleBottom: contentBottom,
            subtitleTop: subtitleTop,
            subtitleBottom: subtitleBottom,
        )
    }
}

public struct ValueHeaderView: View {
    public enum Style {
        case standard
        case wowPay
        case wowPayOverview
        case wowPayToken
        case wowPayConfirmation
    }

    private let style: Style
    private let model: any ValueHeaderViewModel

    @Binding var isPrivacyEnabled: Bool

    private let titleActionType: HeaderTitleActionType
    private let spacing: ValueHeaderViewSpacing
    private let networkTitle: String?
    private let onHeaderAction: HeaderButtonAction?
    private let onSubtitleAction: VoidAction
    private let onInfoAction: VoidAction

    public init(
        model: any ValueHeaderViewModel,
        isPrivacyEnabled: Binding<Bool>,
        titleActionType: HeaderTitleActionType,
        spacing: ValueHeaderViewSpacing = .standard,
        style: Style = .standard,
        onHeaderAction: HeaderButtonAction?,
        networkTitle: String? = nil,
        onSubtitleAction: VoidAction = nil,
        onInfoAction: VoidAction,
    ) {
        self.style = style
        self.model = model
        _isPrivacyEnabled = isPrivacyEnabled
        self.titleActionType = titleActionType
        self.spacing = spacing
        self.networkTitle = networkTitle
        self.onHeaderAction = onHeaderAction
        self.onSubtitleAction = onSubtitleAction
        self.onInfoAction = onInfoAction
    }

    public var body: some View {
        switch style {
        case .standard:
            standardContent
        case .wowPay:
            VStack(spacing: Spacing.wowPay.page) {
                wowPayBalance
                headerActions
            }
        case .wowPayOverview, .wowPayToken:
            VStack(spacing: Spacing.wowPay.page) {
                wowPayBalance
                if model.isWatchWallet {
                    headerActions
                }
            }
        case .wowPayConfirmation:
            VStack(spacing: Spacing.space12) {
                if let assetImage = model.assetImage {
                    AssetImageView(assetImage: assetImage, size: Sizing.wowPay.detailAssetIcon)
                }
                titleView
                    .font(.wowPay.assetBalance)
                    .foregroundStyle(Colors.WowPay.heading)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                if let subtitle = model.subtitle {
                    subtitleView(subtitle)
                }
            }
            .frame(maxWidth: .infinity)
            .wowPayCard()
        }
    }

    private var isOverview: Bool { style == .wowPayOverview || style == .wowPayToken }

    private var balanceMinimumHeight: CGFloat {
        switch style {
        case .wowPayToken: Sizing.wowPay.tokenBalanceHeight
        case .wowPayOverview: Sizing.wowPay.assetOverviewBalanceHeight
        case .standard, .wowPay, .wowPayConfirmation: Sizing.wowPay.balanceMinimumHeight
        }
    }

    private var wowPayBalance: some View {
        VStack(alignment: .leading, spacing: isOverview ? Spacing.small : Spacing.wowPay.balanceTitleGap) {
            VStack(alignment: .leading, spacing: Spacing.wowPay.balanceTitleGap) {
                Text(isOverview ? Localized.wowpayMyBalance : Localized.Asset.balances)
                    .font(.wowPay.balanceLabel)
                Group {
                    if isOverview, let symbol = model.amountSymbol {
                        HStack(alignment: .firstTextBaseline, spacing: .zero) {
                            Text(model.amountTitle).font(style == .wowPayToken ? .wowPay.tokenBalance : .wowPay.assetBalance)
                            Text(" " + symbol).font(style == .wowPayToken ? .wowPay.tokenBalanceSymbol : .wowPay.assetBalanceSymbol)
                        }
                        .accessibilityElement(children: .combine)
                    } else {
                        titleView
                    }
                }
                .numericTransition(for: model.title)
                .font(.wowPay.assetBalance)
                .tracking(WowPayTypography.balanceTracking)
                .minimumScaleFactor(0.5)
                .lineLimit(1)
            }
            if let subtitle = model.subtitle {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.small) {
                    subtitleView(subtitle)
                        .numericTransition(for: model.subtitle)
                    if style == .wowPayToken, let networkTitle {
                        Spacer(minLength: .zero)
                        Text(networkTitle)
                            .font(.wowPay.balanceFootnote)
                            .foregroundStyle(Colors.WowPay.onBrandSecondary)
                            .multilineTextAlignment(.trailing)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, Spacing.wowPay.balancePadding)
        .padding(.horizontal, Spacing.wowPay.balanceHorizontalPadding)
        .frame(minHeight: balanceMinimumHeight)
        .foregroundStyle(Colors.WowPay.onBrand)
        .background(WowPayGradients.balance, in: RoundedRectangle(cornerRadius: Sizing.wowPay.cardCorner))
        .shadow(color: Colors.WowPay.balanceShadow, radius: Sizing.wowPay.balanceShadowRadius, y: Sizing.wowPay.balanceShadowOffset)
    }

    private var standardContent: some View {
        VStack(spacing: .zero) {
            if let assetImage = model.assetImage {
                AssetImageView(
                    assetImage: assetImage,
                    size: .image.semiLarge,
                )
                .padding(.bottom, spacing.assetImageBottom)
            }
            titleView
                .numericTransition(for: model.title)
                .minimumScaleFactor(0.35)
                .font(.app.largeTitle)
                .foregroundStyle(Colors.black)
                .lineLimit(1)
                .padding(.bottom, spacing.titleBottom)

            if let subtitle = model.subtitle {
                subtitleView(subtitle)
                    .numericTransition(for: model.subtitle)
                    .padding(.top, spacing.subtitleTop)
                    .padding(.bottom, spacing.subtitleBottom)
            }

            headerActions
                .padding(.top, .space8)
        }
    }

    @ViewBuilder
    private var headerActions: some View {
        switch model.isWatchWallet {
        case true:
            Button {
                onInfoAction?()
            } label: {
                HStack {
                    Images.System.eye

                    Spacer(minLength: .zero)

                    Text(Localized.Wallet.Watch.Tooltip.title)
                        .foregroundStyle(style == .standard ? Colors.black : Colors.WowPay.heading)
                        .font(style == .standard ? .native(.callout) : .wowPay.subtitle)
                        .multilineTextAlignment(.center)
                        .fixedSize(horizontal: false, vertical: true)

                    Spacer(minLength: .zero)

                    Images.System.info
                        .tint(style == .standard ? Colors.black : Colors.WowPay.brand)
                }
                .padding()
                .frame(maxWidth: .infinity)
                .background(style == .standard ? Colors.listStyleColor : Colors.WowPay.pale)
                .cornerRadius(style == .standard ? .medium : Sizing.wowPay.controlCorner)
            }

        case false:
            HeaderButtonsView(buttons: model.buttons, style: style == .wowPay ? .wowPay : .standard, action: onHeaderAction)
        }
    }

    @ViewBuilder
    private func subtitleView(_ subtitle: String) -> some View {
        let content = HStack(spacing: Spacing.space6) {
            PrivacyText(
                subtitle,
                isEnabled: $isPrivacyEnabled,
            )
            .font(style == .standard ? .native(.app.headline) : .wowPay.balanceFootnote)
            .foregroundStyle(subtitleColor)

            if let subtitleImage = model.subtitleImage {
                subtitleImage
                    .font(.footnote)
                    .foregroundStyle(Colors.secondaryText)
            }
        }

        if let onSubtitleAction {
            Button(action: onSubtitleAction) {
                content
            }
        } else {
            content
        }
    }

    private var subtitleColor: Color {
        switch style {
        case .standard: model.subtitleColor
        case .wowPay, .wowPayOverview, .wowPayToken: Colors.WowPay.onBrandSecondary
        case .wowPayConfirmation: Colors.WowPay.secondaryText
        }
    }

    @ViewBuilder
    private var titleView: some View {
        switch titleActionType {
        case .privacyToggle:
            PrivacyToggleView(model.title, isEnabled: $isPrivacyEnabled)
        case let .action(action):
            Button(action: action) {
                PrivacyText(model.title, isEnabled: $isPrivacyEnabled)
            }
        case .none:
            Text(model.title)
        }
    }
}

// MARK: - Previews

#Preview {
    let model = WalletHeaderViewModel(
        totalValue: TotalFiatValue(value: 1000, pnlAmount: 50, pnlPercentage: 5.26),
        currencyCode: Currency.usd.rawValue,
        showsPnl: true,
        actions: .buttons(buttons: []),
    )

    ValueHeaderView(
        model: model,
        isPrivacyEnabled: .constant(false),
        titleActionType: .privacyToggle,
        onHeaderAction: .none,
        onInfoAction: .none,
    )
}
