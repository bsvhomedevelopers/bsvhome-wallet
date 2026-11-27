import Localization
import Style
import SwiftUI

struct BsvMultisigSignatureProgress: View {
    let signedMembers: Int
    let requiredSignatures: UInt32

    var body: some View {
        GeometryReader { geometry in
            Capsule().fill(Colors.WowPay.brand)
                .frame(width: geometry.size.width * min(Double(signedMembers) / Double(requiredSignatures), 1))
        }
        .frame(height: Sizing.wowPay.multisigProgress)
        .background(Colors.WowPay.serviceSurface, in: Capsule())
        .accessibilityLabel(Localized.multisigSigningProgress)
        .accessibilityValue("\(signedMembers)/\(requiredSignatures)")
    }
}
