import struct BsvHomeCore.BsvHomeBsvMultisigMember
import Localization
import Style
import SwiftUI

struct BsvMultisigMemberRow: View {
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    let member: BsvHomeBsvMultisigMember
    var signedMembers: [String]?
    var onCopy: ((String) -> Void)?

    private var isConfirmed: Bool {
        if let signedMembers {
            return member.publicKey.map(signedMembers.contains) ?? false
        }
        return member.status == .confirmed
    }

    private var status: String {
        if signedMembers != nil {
            return isConfirmed ? Localized.multisigSigned : Localized.multisigUnsigned
        }
        return member.statusTitle
    }

    var body: some View {
        VStack(spacing: .zero) {
            Group {
                if dynamicTypeSize.isAccessibilitySize {
                    VStack(alignment: .leading, spacing: Spacing.small) {
                        Text(member.name.isEmpty ? Localized.multisigMemberName(Int(member.slot + 1)) : member.name).font(.subheadline.weight(.bold))
                        BsvMultisigChip(title: status, confirmed: isConfirmed)
                        identity
                    }
                } else {
                    HStack(spacing: Spacing.wowPay.grid) {
                        Text(String(member.slot + 1))
                            .font(.headline)
                            .foregroundStyle(Colors.WowPay.brand)
                            .frame(width: Sizing.wowPay.multisigAvatar, height: Sizing.wowPay.multisigAvatar)
                            .background(Colors.WowPay.serviceSurface, in: Circle())
                        VStack(alignment: .leading, spacing: Spacing.space4) {
                            Text(member.name.isEmpty ? Localized.multisigMemberName(Int(member.slot + 1)) : member.name).font(.subheadline.weight(.bold))
                            identity
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        BsvMultisigChip(title: status, confirmed: isConfirmed)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.vertical, Spacing.wowPay.multisigMember)
            Divider().overlay(Colors.WowPay.border)
        }
    }

    private var identity: some View {
        VStack(alignment: .leading, spacing: Spacing.space4) {
            if let publicKey = member.publicKey {
                copyableValue(publicKey)
            } else {
                Text(Localized.multisigPublicKeyPending).font(.caption2)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            if let address = member.address {
                copyableValue(address)
            }
        }
    }

    private func copyableValue(_ value: String) -> some View {
        HStack(alignment: .top, spacing: Spacing.space4) {
            Text(value).font(.caption2).foregroundStyle(Colors.WowPay.secondaryText)
                .textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let onCopy {
                Button(Localized.Common.copy, systemImage: SystemImage.copy) { onCopy(value) }
                    .labelStyle(.iconOnly).buttonStyle(.plain)
                    .frame(minWidth: Spacing.wowPay.minimumTapTarget, minHeight: Spacing.wowPay.minimumTapTarget)
                    .foregroundStyle(Colors.WowPay.brand)
                    .accessibilityValue(value)
            }
        }
    }
}
