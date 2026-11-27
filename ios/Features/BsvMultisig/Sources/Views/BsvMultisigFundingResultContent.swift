import struct BsvHomeCore.BsvHomeBsvMultisigFundingResult
import Localization
import Style
import SwiftUI

struct BsvMultisigFundingResultContent: View {
    let result: BsvHomeBsvMultisigFundingResult
    let targetName: String
    let amounts: BsvMultisigAmountFormatter
    let onCopy: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.small) {
            Text(amounts.amount(result.amountSatoshis, symbol: true)).font(.wowPay.multisigTransactionAmount).frame(maxWidth: .infinity)
            BsvMultisigValueRow(title: Localized.multisigFundingPayer, value: result.payerWalletName)
            BsvMultisigValueRow(title: Localized.multisigFundingTotal, value: amounts.amount(result.totalSatoshis, symbol: true))
            BsvMultisigValueRow(title: Localized.multisigFundingTarget, value: targetName)
            BsvMultisigValueRow(title: Localized.multisigTransactionId, value: result.transactionId)
            Button(Localized.Common.copy, action: onCopy).buttonStyle(.wowPaySecondary())
        }
        .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
    }
}
