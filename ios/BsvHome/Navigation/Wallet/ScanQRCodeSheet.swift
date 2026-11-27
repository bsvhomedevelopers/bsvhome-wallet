// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Primitives
import QRScanner
import SwiftUI

extension View {
    func scanQRCodeSheet(isPresented: Binding<Bool>, action: StringAction) -> some View {
        modifier(ScanQRCodeSheet(isPresented: isPresented, action: action))
    }
}

private struct ScanQRCodeSheet: ViewModifier {
    @Binding var isPresented: Bool

    @State private var code: String?

    let action: StringAction

    func body(content: Content) -> some View {
        content.sheet(isPresented: $isPresented, onDismiss: onDismiss) {
            ScanQRCodeNavigationStack(scanType: .universal, action: { code = $0 })
                .orientationLock(.portrait)
        }
    }

    private func onDismiss() {
        guard let code else { return }
        self.code = .none
        action?(code)
    }
}
