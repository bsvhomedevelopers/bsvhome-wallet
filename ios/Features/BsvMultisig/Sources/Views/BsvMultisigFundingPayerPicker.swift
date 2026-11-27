import struct BsvHomeCore.BsvHomeBsvMultisigFundingPayer
import Localization
import Style
import SwiftUI

struct BsvMultisigFundingPayerPicker: View {
    let payers: [BsvHomeBsvMultisigFundingPayer]
    let selectedWalletId: String?
    let onSelect: (BsvHomeBsvMultisigFundingPayer) -> Void

    var body: some View {
        Text(Localized.multisigFundingPayer).font(.headline)
        ForEach(payers, id: \.walletId) { payer in
            Button { onSelect(payer) } label: {
                HStack(alignment: .top, spacing: Spacing.small) {
                    Image(systemName: selectedWalletId == payer.walletId ? "largecircle.fill.circle" : "circle").foregroundStyle(Colors.WowPay.brand)
                    VStack(alignment: .leading, spacing: Spacing.space4) {
                        Text(payer.name).font(.subheadline.weight(.bold))
                        Text(payer.address).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
                    }
                    Spacer(minLength: .zero)
                }
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(selectedWalletId == payer.walletId ? .isSelected : [])
            .accessibilityIdentifier("bsvFundingPayer-" + payer.walletId)
        }
    }
}
