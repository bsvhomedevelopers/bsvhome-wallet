import Components
import Foundation
import enum Primitives.QRScanType
import SwiftUI

@Observable
@MainActor
final class BsvMultisigScannerPresentation {
    struct Presentation: Identifiable {
        let id: UUID
        let scanType: QRScanType
    }

    private struct Request {
        let presentation: Presentation
        let navigation: NavigationPathState
        let observerId: UUID
        let isActive: @MainActor () -> Bool
        let onCode: @MainActor (String) -> Void
    }

    private var request: Request?

    var presentation: Presentation? {
        request?.presentation
    }

    var binding: Binding<Presentation?> {
        let presentedId = presentation?.id
        return Binding(get: { self.presentation }, set: { value in
            if value == nil, let presentedId {
                self.cancel(id: presentedId)
            }
        })
    }

    func present(navigation: NavigationPathState, isActive: @escaping @MainActor () -> Bool, scanType: QRScanType, onCode: @escaping @MainActor (String) -> Void) {
        cancel()
        guard isActive() else { return }
        let presentation = Presentation(id: UUID(), scanType: scanType)
        let observerId = navigation.observeRemoval { [weak self] in
            self?.cancel(id: presentation.id)
        }
        request = Request(presentation: presentation, navigation: navigation, observerId: observerId, isActive: isActive, onCode: onCode)
    }

    func complete(id: UUID, value: String) {
        guard let request, request.presentation.id == id else { return }
        let isActive = request.isActive()
        cancel(id: id)
        if isActive {
            request.onCode(value)
        }
    }

    func cancel(id: UUID) {
        guard request?.presentation.id == id else { return }
        cancel()
    }

    func cancel() {
        guard let request else { return }
        self.request = nil
        request.navigation.removeRemovalObserver(request.observerId)
    }
}
