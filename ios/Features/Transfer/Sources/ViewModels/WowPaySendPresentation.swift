import Components
import Foundation
import SwiftUI

@Observable
@MainActor
public final class WowPaySendPresentation {
    public private(set) var id: UUID?
    public private(set) var model: WowPaySendViewModel?
    public let confirmation = ConfirmTransferPresentation()
    private var navigation: NavigationPathState?
    private var previousPath: NavigationPath?
    private var removalObserver: UUID?
    private var onRetire: (@MainActor () -> Void)?

    public init() {}

    public func present(id: UUID, model: WowPaySendViewModel, navigation: NavigationPathState, onRetire: (@MainActor () -> Void)? = nil) {
        dismiss()
        previousPath = navigation.path
        self.id = id
        self.model = model
        self.navigation = navigation
        self.onRetire = onRetire
        navigation.append(WowPaySendDestination(id: id))
        removalObserver = navigation.observeRemoval { [weak self] in
            if self?.id == id {
                self?.retire()
            }
        }
    }

    public func isCurrent(_ id: UUID) -> Bool {
        self.id == id
    }

    public func dismiss(id: UUID) {
        guard self.id == id else { return }
        dismiss()
    }

    public func dismiss() {
        let navigation = navigation
        let previousPath = previousPath
        retire()
        if let navigation, let previousPath {
            navigation.binding.wrappedValue = previousPath
        }
    }

    private func retire() {
        let onRetire = onRetire
        self.onRetire = nil
        let model = model
        self.model = nil
        id = nil
        if let removalObserver {
            navigation?.removeRemovalObserver(removalObserver)
        }
        removalObserver = nil
        navigation = nil
        previousPath = nil
        model?.cancel()
        confirmation.dismiss()
        onRetire?()
    }
}
