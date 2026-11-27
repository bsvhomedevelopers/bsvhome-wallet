import Components
import Localization
import Primitives
import Style
import SwiftUI

public struct WowPayAssetTransactions: View {
    private let transactions: [TransactionViewModel]
    private let currency: String
    private let emptyTitle: String
    private let state: StateViewType<Void>
    private let onRetry: (() -> Void)?

    public init(transactions: [TransactionViewModel], currency: String, emptyTitle: String = Localized.Activity.State.Empty.title, state: StateViewType<Void> = .data(()), onRetry: (() -> Void)? = nil) {
        self.transactions = transactions
        self.currency = currency
        self.emptyTitle = emptyTitle
        self.state = state
        self.onRetry = onRetry
    }

    public var body: some View {
        VStack(spacing: .zero) {
            switch state {
            case .noData, .loading:
                LoadingView(tint: Colors.WowPay.brand)
                    .frame(maxWidth: .infinity)
                    .padding(Spacing.wowPay.section)
                    .accessibilityLabel(Localized.Common.loading)
            case .error:
                StateEmptyView(
                    title: transactions.isEmpty ? Localized.Errors.errorOccurred : Localized.Errors.balancesActivityOutdated,
                    titleTextStyle: .wowPay.value,
                ) {
                    if let onRetry {
                        Button(Localized.Common.tryAgain, action: onRetry)
                            .buttonStyle(.wowPaySecondary())
                    }
                }
                .padding(Spacing.wowPay.section)
            case .data:
                if transactions.isEmpty {
                    Text(emptyTitle)
                        .font(.wowPay.subtitle)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                        .frame(maxWidth: .infinity)
                        .padding(Spacing.wowPay.section)
                }
            }
            if transactions.isNotEmpty {
                ForEach(transactions) { transaction in
                    NavigationLink(value: Scenes.Transaction(transaction: transaction.transaction)) {
                        TransactionView(model: transaction, currency: currency, style: .wowPay, appearance: .assetDetail)
                            .padding(Spacing.wowPay.transactionRowPadding)
                    }
                    .buttonStyle(.plain)
                    if transaction.id != transactions.last?.id {
                        Colors.WowPay.border.frame(height: Spacing.space1)
                    }
                }
            }
        }
        .wowPayCard(padding: .zero, cornerRadius: Sizing.wowPay.rowCorner)
    }
}
