import struct BsvHomeCore.BsvHomeBsvMultisigFundingPreparation
import Localization
import Style
import SwiftUI

struct BsvMultisigFundingReviewContent: View {
    let preparation: BsvHomeBsvMultisigFundingPreparation
    let amounts: BsvMultisigAmountFormatter

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.small) {
            Text(amounts.amount(preparation.amountSatoshis, symbol: true)).font(.wowPay.multisigTransactionAmount).frame(maxWidth: .infinity)
            BsvMultisigValueRow(title: Localized.multisigFundingPayer, value: preparation.payerWalletName)
            BsvMultisigValueRow(title: Localized.Common.address, value: preparation.senderAddress)
            BsvMultisigValueRow(title: Localized.multisigFundingTarget, value: preparation.targetName)
            BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
            BsvMultisigValueRow(title: Localized.Transfer.networkFee, value: amounts.amount(preparation.feeSatoshis, symbol: true))
            BsvMultisigValueRow(title: Localized.multisigFundingTotal, value: amounts.amount(preparation.totalSatoshis, symbol: true))
            BsvMultisigValueRow(title: Localized.multisigChangeOutput, value: amounts.amount(preparation.changeSatoshis, symbol: true))
            BsvMultisigValueRow(title: Localized.multisigInput, value: String(preparation.inputCount))
        }
        .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
        VStack(alignment: .leading, spacing: Spacing.small) {
            switch preparation.target {
            case let .member(_, _, members, threshold):
                BsvMultisigValueRow(title: Localized.multisigThreshold, value: "\(threshold)/\(members.count)")
                BsvMultisigValueRow(title: Localized.multisigConfigurationId, value: preparation.configurationHash)
                ForEach(members, id: \.slot) { BsvMultisigMemberRow(member: $0) }
            case let .public(request):
                BsvMultisigValueRow(title: Localized.multisigThreshold, value: "\(request.threshold)/\(request.publicKeys.count)")
                BsvMultisigValueRow(title: Localized.multisigConfigurationId, value: preparation.configurationHash)
                ForEach(Array(request.publicKeys.enumerated()), id: \.offset) { _, key in
                    BsvMultisigValueRow(title: Localized.multisigPublicKey, value: key)
                }
            }
        }
        .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
    }
}
