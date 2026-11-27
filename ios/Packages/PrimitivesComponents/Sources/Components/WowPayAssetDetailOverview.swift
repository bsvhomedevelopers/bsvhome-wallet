import Components
import Localization
import Style
import SwiftUI

public struct WowPayAssetDetailOverview<Balance: View, QRCard: View, Transactions: View, Actions: View>: View {
    private let balance: Balance
    private let qrCard: QRCard
    private let transactions: Transactions
    private let actions: Actions
    private let onPaymail: (() -> Void)?
    private let paymailHandle: String?
    private let paymailStatus: String
    private let onPaymailHelp: () -> Void
    private let paymailInteraction: WowPayPaymailCard.InteractionStyle
    private let onViewAll: () -> Void

    private var isWholeCard: Bool { paymailInteraction == .wholeCard }

    public init(onPaymail: (() -> Void)?, paymailHandle: String? = nil, paymailStatus: String = Localized.wowpayPaymailSetup, paymailInteraction: WowPayPaymailCard.InteractionStyle = .actionButton, onPaymailHelp: @escaping () -> Void, onViewAll: @escaping () -> Void, @ViewBuilder balance: () -> Balance, @ViewBuilder qrCard: () -> QRCard, @ViewBuilder transactions: () -> Transactions, @ViewBuilder actions: () -> Actions) {
        self.onPaymail = onPaymail
        self.paymailHandle = paymailHandle
        self.paymailStatus = paymailStatus
        self.onPaymailHelp = onPaymailHelp
        self.paymailInteraction = paymailInteraction
        self.onViewAll = onViewAll
        self.balance = balance()
        self.qrCard = qrCard()
        self.transactions = transactions()
        self.actions = actions()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.page) {
            VStack(alignment: .leading, spacing: isWholeCard ? .zero : Spacing.wowPay.section) {
                balance
                if onPaymail != nil || QRCard.self != EmptyView.self {
                    HStack(alignment: .top, spacing: Spacing.wowPay.grid) {
                        if let onPaymail {
                            WowPayPaymailCard(
                                title: Localized.wowpayReceivePaymailTitle,
                                address: paymailHandle ?? (paymailInteraction == .wholeCard ? "" : paymailStatus),
                                description: Localized.wowpayPaymailCollectDescription,
                                actionTitle: Localized.wowpayPaymailManage,
                                onSelect: onPaymail,
                                onHelp: onPaymailHelp,
                                interactionStyle: paymailInteraction,
                            )
                        }
                        qrCard
                    }
                    .padding(.top, isWholeCard ? Spacing.wowPay.overviewBalanceToPaymail : .zero)
                }
                VStack(alignment: .leading, spacing: Spacing.wowPay.servicePadding) {
                    HStack {
                        Text(Localized.wowpayRecentTransactions).font(.wowPay.sectionTitle).foregroundStyle(Colors.WowPay.heading)
                        Spacer()
                        Button { onViewAll() } label: {
                            HStack(spacing: .tiny) {
                                Text(Localized.wowpayViewAll)
                                Images.System.chevronRight
                            }
                            .font(.wowPay.rowTitle)
                            .foregroundStyle(Colors.WowPay.brand)
                        }
                        .accessibilityIdentifier("wowPayAllTransactions")
                    }
                    transactions
                }
                .padding(.top, isWholeCard ? (onPaymail == nil ? Spacing.wowPay.overviewBalanceToHistory : Spacing.wowPay.overviewPaymailToHistory) : .zero)
            }
            actions
        }
    }
}
