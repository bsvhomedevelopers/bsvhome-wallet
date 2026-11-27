import SwiftUI

struct MpcBackupPresentation: ViewModifier {
    let model: MpcWalletViewModel
    @Binding var ticket: MpcBackupExportTicket?
    @State private var retained: [UUID: MpcBackupExportTicket] = [:]

    func body(content: Content) -> some View {
        content
            .onChange(of: ticket, initial: true) { old, current in
                if let old {
                    retained[old.id] = old
                }
                if let current {
                    retained[current.id] = current
                }
            }
            .onChange(of: model.backup?.commitment) { _, _ in
                if let current = ticket, !model.isCurrentExport(current), ticket?.id == current.id {
                    ticket = nil
                }
            }
            .sheet(item: presentation, onDismiss: releaseRetired) { current in
                MpcBackupDocumentPicker(ticket: current) { completed, outcome, cleanupFailed in
                    model.backupExportFinished(completed, outcome: outcome, cleanupFailed: cleanupFailed)
                    if ticket?.id == completed.id {
                        ticket = nil
                    }
                }
            }
    }

    private var presentation: Binding<MpcBackupExportTicket?> {
        let identity = ticket?.id
        return Binding(get: { ticket?.id == identity ? ticket : nil }, set: { value in
            if value == nil, ticket?.id == identity {
                ticket = nil
            }
        })
    }

    private func releaseRetired() {
        for current in Array(retained.values) where current.id != ticket?.id {
            model.discardBackupExport(current)
            retained[current.id] = nil
        }
    }
}
