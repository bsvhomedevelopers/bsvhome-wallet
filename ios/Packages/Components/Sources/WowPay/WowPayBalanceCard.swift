import Style
import SwiftUI

public struct WowPayBalanceCard: View {
    public enum VisibilityControl {
        case toggle
        case hidden
    }

    private let title: String
    private let value: String
    private let subtitle: String?
    private let secondaryAmount: String?
    private let coinImage: Image?
    @Binding private var isHidden: Bool
    private let action: (() -> Void)?
    private let onSelectCoin: (() -> Void)?
    private let visibilityControl: VisibilityControl

    public init(
        title: String,
        value: String,
        subtitle: String? = nil,
        secondaryAmount: String? = nil,
        isHidden: Binding<Bool>,
        action: (() -> Void)? = nil,
        coinImage: Image? = nil,
        onSelectCoin: (() -> Void)? = nil,
        visibilityControl: VisibilityControl = .toggle,
    ) {
        self.title = title
        self.value = value
        self.subtitle = subtitle
        self.secondaryAmount = secondaryAmount
        self.coinImage = coinImage
        _isHidden = isHidden
        self.action = action
        self.onSelectCoin = onSelectCoin
        self.visibilityControl = visibilityControl
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.balanceTitleGap) {
            titleContent

            if let subtitle, let onSelectCoin {
                selectableBalance(subtitle: subtitle, onSelectCoin: onSelectCoin)
            } else if let action {
                Button(action: action) {
                    balance
                }
                .buttonStyle(.plain)
            } else {
                balance
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, Spacing.wowPay.balancePadding)
        .padding(.horizontal, Spacing.wowPay.balanceHorizontalPadding)
        .frame(minHeight: Sizing.wowPay.balanceMinimumHeight)
        .foregroundStyle(Colors.WowPay.onBrand)
        .background(WowPayGradients.balance, in: RoundedRectangle(cornerRadius: Sizing.wowPay.cardCorner))
        .shadow(color: Colors.WowPay.balanceShadow, radius: Sizing.wowPay.balanceShadowRadius, y: Sizing.wowPay.balanceShadowOffset)
    }

    @ViewBuilder
    private var titleContent: some View {
        switch visibilityControl {
        case .toggle:
            HStack(spacing: .small) {
                Text(title)
                    .font(.wowPay.balanceLabel)
                Image(isHidden ? "wowpay-eye-off" : "wowpay-eye")
                    .renderingMode(.template)
                    .resizable()
                    .scaledToFit()
                    .frame(width: Sizing.wowPay.balanceVisibilityIcon, height: Sizing.wowPay.balanceVisibilityIcon)
            }
            .accessibilityHidden(true)
            .overlay {
                Button {
                    isHidden.toggle()
                } label: {
                    Color.clear
                        .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(title)
            }
        case .hidden:
            Text(title).font(.wowPay.balanceLabel)
        }
    }

    private var balance: some View {
        VStack(alignment: .leading, spacing: .small) {
            mainAmount
            if subtitle != nil || secondaryAmount != nil {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.small) {
                    if let subtitle {
                        PrivacyText(subtitle, isEnabled: $isHidden, layout: .preserved)
                    }
                    if let secondaryAmount {
                        Spacer(minLength: .zero)
                        PrivacyText(secondaryAmount, isEnabled: $isHidden, layout: .preserved)
                            .multilineTextAlignment(.trailing)
                    }
                }
                .font(.wowPay.balanceFootnote)
                .foregroundStyle(Colors.WowPay.onBrandSecondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
    }

    private func selectableBalance(subtitle: String, onSelectCoin: @escaping () -> Void) -> some View {
        VStack(alignment: .leading, spacing: .small) {
            balanceAction { mainAmount }
            HStack(spacing: Spacing.small) {
                Button(action: onSelectCoin) {
                    HStack(spacing: Spacing.small) {
                        if let coinImage {
                            coinImage
                                .resizable()
                                .scaledToFit()
                                .frame(width: Sizing.wowPay.pairCoin, height: Sizing.wowPay.pairCoin)
                                .accessibilityHidden(true)
                        }
                        Text(subtitle)
                            .lineLimit(1)
                    }
                    .frame(minHeight: Sizing.wowPay.minimumTapTarget)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(subtitle)

                if let secondaryAmount {
                    balanceAction {
                        HStack(spacing: .zero) {
                            Spacer(minLength: .zero)
                            PrivacyText(secondaryAmount, isEnabled: $isHidden, layout: .preserved)
                                .multilineTextAlignment(.trailing)
                        }
                        .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.minimumTapTarget)
                        .contentShape(Rectangle())
                    }
                } else {
                    Spacer(minLength: .zero)
                }
            }
            .font(.wowPay.balanceFootnote)
            .foregroundStyle(Colors.WowPay.onBrandSecondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var mainAmount: some View {
        PrivacyText(value, isEnabled: $isHidden, layout: .preserved)
            .font(.wowPay.balance)
            .tracking(WowPayTypography.balanceTracking)
            .minimumScaleFactor(0.5)
            .lineLimit(1)
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
    }

    @ViewBuilder
    private func balanceAction(@ViewBuilder content: () -> some View) -> some View {
        if let action {
            Button(action: action, label: content)
                .buttonStyle(.plain)
        } else {
            content()
        }
    }
}

#Preview {
    @Previewable @State var isHidden = false
    WowPayBalanceCard(
        title: "Total assets (USD)",
        value: "$2,580.0000",
        subtitle: "BSV",
        secondaryAmount: "≈ $2,580.0000",
        isHidden: $isHidden,
        action: {},
        coinImage: Image(systemName: "bitcoinsign.circle.fill"),
        onSelectCoin: {},
    )
    .padding(Spacing.wowPay.page)
    .wowPayBackground()
}
