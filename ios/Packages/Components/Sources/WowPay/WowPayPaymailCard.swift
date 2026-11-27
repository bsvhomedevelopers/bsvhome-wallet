import Style
import SwiftUI

public struct WowPayPaymailCard: View {
    public enum InteractionStyle: Equatable, Sendable {
        case actionButton
        case wholeCard
    }

    private let title: String
    private let address: String
    private let description: String
    private let actionTitle: String
    private let onSelect: () -> Void
    private let onHelp: () -> Void
    private let interactionStyle: InteractionStyle

    public init(title: String, address: String, description: String, actionTitle: String, onSelect: @escaping () -> Void, onHelp: @escaping () -> Void, interactionStyle: InteractionStyle = .actionButton) {
        self.title = title
        self.address = address
        self.description = description
        self.actionTitle = actionTitle
        self.onSelect = onSelect
        self.onHelp = onHelp
        self.interactionStyle = interactionStyle
    }

    public var body: some View {
        switch interactionStyle {
        case .actionButton:
            card
        case .wholeCard:
            Button(action: onSelect) { card }
                .buttonStyle(.plain)
                .accessibilityIdentifier("wowPayManagePaymail")
        }
    }

    private var card: some View {
        VStack(alignment: .leading, spacing: isWholeCard ? Spacing.space12 : Spacing.wowPay.grid) {
            HStack(alignment: isWholeCard ? .center : .top, spacing: isWholeCard ? Spacing.space12 : .small) {
                Image("wowpay-paymail")
                    .renderingMode(.template)
                    .resizable()
                    .scaledToFit()
                    .frame(width: isWholeCard ? Sizing.wowPay.paymailOverviewIcon : Sizing.wowPay.paymailIcon, height: isWholeCard ? Sizing.wowPay.paymailOverviewIcon : Sizing.wowPay.paymailIcon)
                    .foregroundStyle(Colors.WowPay.brand)
                    .frame(width: isWholeCard ? Sizing.wowPay.paymailOverviewAvatar : Sizing.wowPay.paymailAvatar, height: isWholeCard ? Sizing.wowPay.paymailOverviewAvatar : Sizing.wowPay.paymailAvatar)
                    .background {
                        if isWholeCard {
                            RoundedRectangle(cornerRadius: Sizing.wowPay.serviceIconCorner).fill(Colors.WowPay.pale)
                        } else {
                            Circle().fill(Colors.WowPay.pale)
                        }
                    }
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: isWholeCard ? .space4 : .space6) {
                    HStack(spacing: .tiny) {
                        Text(title)
                            .font(isWholeCard ? .wowPay.paymailOverviewTitle : .wowPay.selectionTitle)
                            .accessibilityAddTraits(.isHeader)
                        switch interactionStyle {
                        case .actionButton:
                            Button(action: onHelp) { helpImage }
                                .buttonStyle(.plain)
                                .accessibilityLabel(title)
                        case .wholeCard:
                            EmptyView()
                        }
                    }
                    if address.isEmpty == false {
                        if isWholeCard {
                            Text(verbatim: address)
                                .font(.wowPay.paymailOverviewAddress)
                                .foregroundStyle(Colors.WowPay.brand)
                                .fixedSize(horizontal: false, vertical: true)
                        } else {
                            Text(verbatim: address)
                                .font(.wowPay.serviceCaption)
                                .foregroundStyle(Colors.WowPay.secondaryText)
                                .lineLimit(1)
                                .minimumScaleFactor(0.8)
                        }
                    }
                    Text(description)
                        .font(isWholeCard ? .wowPay.caption : .wowPay.serviceCaption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            if !isWholeCard {
                Spacer(minLength: .zero)
            }
            switch interactionStyle {
            case .actionButton:
                Button(action: onSelect) { actionLabel }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("wowPayManagePaymail")
            case .wholeCard:
                actionLabel
            }
        }
        .foregroundStyle(Colors.WowPay.heading)
        .padding(isWholeCard ? Spacing.space16 : Spacing.wowPay.servicePadding)
        .frame(maxWidth: .infinity, minHeight: isWholeCard ? nil : Sizing.wowPay.paymailCardHeight, alignment: .topLeading)
        .wowPayCard(padding: .zero, cornerRadius: isWholeCard ? Sizing.wowPay.cardCorner : Sizing.wowPay.rowCorner)
        .contentShape(Rectangle())
    }

    private var helpImage: some View {
        Image("wowpay-help").renderingMode(.template).resizable().scaledToFit()
            .frame(width: Sizing.wowPay.paymailHelpIcon, height: Sizing.wowPay.paymailHelpIcon)
            .foregroundStyle(Colors.WowPay.brand)
            .contentShape(Rectangle().inset(by: -Spacing.space16))
    }

    private var actionLabel: some View {
        Text(actionTitle)
            .font(isWholeCard ? .wowPay.label : .wowPay.paymailAction)
            .foregroundStyle(Colors.WowPay.brand)
            .padding(.vertical, isWholeCard ? Spacing.wowPay.paymailActionVertical : .zero)
            .frame(maxWidth: .infinity, minHeight: isWholeCard ? nil : Sizing.wowPay.paymailActionHeight)
            .background(Colors.WowPay.pale, in: RoundedRectangle(cornerRadius: isWholeCard ? Spacing.space12 : Sizing.wowPay.paymailActionCorner))
            .contentShape(Rectangle().inset(by: -Spacing.tiny))
    }

    private var isWholeCard: Bool { interactionStyle == .wholeCard }
}
