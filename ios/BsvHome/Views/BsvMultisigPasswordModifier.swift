import SwiftUI
import Transfer

private struct BsvMultisigPasswordModifier: ViewModifier {
    let authorization: BsvMultisigPasswordAuthorization?

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
    func bsvMultisigPassword(authorization: BsvMultisigPasswordAuthorization?) -> some View {
        modifier(BsvMultisigPasswordModifier(authorization: authorization))
    }
}
