import Components
import struct BsvHomeCore.BsvHomeBsvMultisigWallet
import Localization
import Style
import SwiftUI

struct BsvMultisigWalletCard: View {
    let model: BsvMultisigViewModel
    let wallet: BsvHomeBsvMultisigWallet
    var onSelect: (() -> Void)?

    private var memberSummary: String {
        let confirmed = wallet.members.filter { $0.status == .confirmed }.count
        return "\(wallet.rule) \(Localized.multisigTitle) · \(confirmed)/\(wallet.memberCount)"
    }

    var body: some View {
        Group {
            if let onSelect {
                Button(action: onSelect) { content }
            } else {
                NavigationLink(value: BsvMultisigRoute.wallet(wallet.id)) { content }
            }
        }
        .buttonStyle(.plain)
        .foregroundStyle(Colors.WowPay.heading)
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            HStack(alignment: .top, spacing: Spacing.small) {
                Text(wallet.name).font(.headline)
                Spacer(minLength: .zero)
                BsvMultisigChip(title: wallet.statusTitle, confirmed: wallet.status == .active)
            }
            Text(Localized.multisigMainnet + " · " + Localized.multisigTitle)
                .font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
            HStack(spacing: Spacing.space4) {
                ForEach(wallet.members, id: \.slot) { member in
                    Text(String(member.slot + 1)).font(.wowPay.multisigChip)
                        .foregroundStyle(Colors.WowPay.brand)
                        .frame(width: Sizing.wowPay.multisigPerson, height: Sizing.wowPay.multisigPerson)
                        .background(Colors.WowPay.serviceSurface, in: Circle())
                }
                Text(memberSummary)
                    .font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
            }
            HStack(alignment: .firstTextBaseline, spacing: Spacing.small) {
                Text(wallet.status == .active ? model.fiat(wallet.balanceSatoshis) ?? model.amount(wallet.balanceSatoshis, symbol: true) : wallet.statusTitle)
                    .font(.headline)
                Spacer(minLength: .zero)
                Text(String(wallet.pendingCount) + " " + Localized.multisigPending + " ›")
                    .font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
    }
}
