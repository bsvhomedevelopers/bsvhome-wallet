import SwiftUI

struct MpcBackupPageLifetime: ViewModifier {
    let model: MpcWalletViewModel
    let walletId: String?
    @Binding var ticket: MpcBackupExportTicket?
    @State private var owner = UUID()

    func body(content: Content) -> some View {
        if let walletId {
            content
                .background { MpcBackupPageObserver(leave: { leave(walletId) }).frame(width: .zero, height: .zero) }
                .onAppear { model.enterBackupPage(owner) }
                .onDisappear {
                    if ticket == nil {
                        leave(walletId)
                    }
                }
        } else {
            content
        }
    }

    private func leave(_ walletId: String) {
        guard model.leaveBackupPage(walletId, owner: owner) else { return }
        if let current = ticket, current.walletId == walletId, ticket?.id == current.id {
            ticket = nil
        }
    }
}
