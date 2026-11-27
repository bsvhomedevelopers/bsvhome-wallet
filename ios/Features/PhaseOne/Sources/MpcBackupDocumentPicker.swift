import SwiftUI
import UIKit

struct MpcBackupDocumentPicker: UIViewControllerRepresentable {
    let ticket: MpcBackupExportTicket
    let completion: @MainActor (MpcBackupExportTicket, MpcBackupExportOutcome, Bool) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(ticket: ticket, completion: completion)
    }

    func makeUIViewController(context: Context) -> UIViewController {
        guard FileManager.default.isReadableFile(atPath: ticket.fileURL.path) else {
            let coordinator = context.coordinator
            Task { coordinator.unavailable() }
            return UIViewController()
        }
        let controller = UIDocumentPickerViewController(forExporting: [ticket.fileURL], asCopy: true)
        controller.delegate = context.coordinator
        controller.allowsMultipleSelection = false
        return controller
    }

    func updateUIViewController(_: UIViewController, context _: Context) {}

    static func dismantleUIViewController(_: UIViewController, coordinator: Coordinator) {
        coordinator.abandon()
    }

    @MainActor
    final class Coordinator: NSObject, UIDocumentPickerDelegate {
        let ticket: MpcBackupExportTicket
        private let completion: @MainActor (MpcBackupExportTicket, MpcBackupExportOutcome, Bool) -> Void
        private var completed = false

        init(ticket: MpcBackupExportTicket, completion: @escaping @MainActor (MpcBackupExportTicket, MpcBackupExportOutcome, Bool) -> Void) {
            self.ticket = ticket
            self.completion = completion
        }

        func documentPicker(_: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
            finish(saved: urls.count == 1)
        }

        func documentPickerWasCancelled(_: UIDocumentPickerViewController) {
            finish(saved: false)
        }

        func finish(saved: Bool) {
            finish(outcome: saved ? .saved : .cancelled)
        }

        func abandon() {
            finish(outcome: .abandoned)
        }

        func unavailable() {
            finish(outcome: .abandoned, unavailable: true)
        }

        private func finish(outcome: MpcBackupExportOutcome, unavailable: Bool = false) {
            guard !completed else { return }
            completed = true
            let cleanupFailed: Bool
            do { try ticket.removeTemporaryFile(); cleanupFailed = unavailable }
            catch { cleanupFailed = true }
            completion(ticket, outcome, cleanupFailed)
        }
    }
}
