// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Style
import SwiftUI

public struct BsvSendSuccessScene: View {
    private let recipient: String
    private let amountText: String
    private let transactionId: String
    private let onBackToAssets: @MainActor () -> Void

    public init(recipient: String, amountText: String, transactionId: String, onBackToAssets: @escaping @MainActor () -> Void) {
        self.recipient = recipient
        self.amountText = amountText
        self.transactionId = transactionId
        self.onBackToAssets = onBackToAssets
    }

    public var body: some View {
        ScrollView {
            VStack(spacing: Spacing.space16) {
                Image("wowpay-check-circle")
                    .renderingMode(.template)
                    .resizable()
                    .scaledToFit()
                    .frame(width: Sizing.wowPay.sendSuccessIcon, height: Sizing.wowPay.sendSuccessIcon)
                    .foregroundStyle(Colors.WowPay.positive)
                    .accessibilityHidden(true)
                Text(amountText)
                    .font(.wowPay.sendReceiptAmount)
                    .foregroundStyle(Colors.WowPay.heading)
                    .multilineTextAlignment(.center)
                    .accessibilityIdentifier("bsvSendSuccessAmount")
                Text(Localized.coldWalletBroadcastSuccess)
                    .font(.wowPay.balanceFootnote)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                receiptField(title: Localized.Transfer.Recipient.title, value: recipient)
                    .accessibilityIdentifier("bsvSendSuccessRecipient")
                receiptField(title: Localized.coldWalletTransactionId, value: transactionId)
                    .accessibilityIdentifier("bsvSendSuccessTransactionId")
            }
            .frame(maxWidth: .infinity)
            .padding(.horizontal, Spacing.wowPay.page)
            .padding(.vertical, Spacing.space24)
        }
        .wowPayBackground(.solid)
        .safeAreaButton(bottomPadding: .space12, maxWidth: .infinity) {
            Button(Localized.Wallet.backToAssets, action: onBackToAssets)
                .buttonStyle(.wowPay())
                .padding(.horizontal, Spacing.wowPay.page)
                .accessibilityIdentifier("bsvSendReturnToAssets")
        }
        .navigationTitle(Localized.Wallet.sendSuccess)
        .navigationBarTitleDisplayMode(.inline)
        .accessibilityIdentifier("bsvSendSuccess")
    }

    private func receiptField(title: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.space6) {
            Text(title)
                .font(.wowPay.label)
                .foregroundStyle(Colors.WowPay.secondaryText)
            Text(value)
                .font(.wowPay.sectionAction)
                .foregroundStyle(Colors.WowPay.heading)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .wowPayCard()
    }
}
