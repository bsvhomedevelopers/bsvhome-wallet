import SwiftUI
import Transfer

private struct BsvColdWalletPasswordModifier: ViewModifier {
    let authorization: BsvColdWalletPasswordAuthorization?

    func body(content: Content) -> some View {
        content.sheet(item: presentation) { password in
            NavigationStack { TransferPasswordScene(model: password) }
                .presentationDetents([.medium, .large])
        }
    }

    private var presentation: Binding<TransferPasswordViewModel?> {
        let requestId = authorization?.requestId
        return Binding(get: { authorization?.presentation }, set: { replacement in
            if replacement == nil, let requestId {
                authorization?.dismissPassword(requestId: requestId)
            }
        })
    }
}

extension View {
    func bsvColdWalletPassword(authorization: BsvColdWalletPasswordAuthorization?) -> some View {
        modifier(BsvColdWalletPasswordModifier(authorization: authorization))
    }
}
