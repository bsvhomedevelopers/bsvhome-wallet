import Components
import Foundation
import class BsvHomeCore.BsvHomeBsvMultisigActionScope

@MainActor
final class BsvMultisigActionLifetime {
    struct Entry {
        let id: UUID
        let scope: BsvHomeBsvMultisigActionScope?
        let navigation: NavigationPathState?
        var observerId: UUID?
        var cancelTask: @MainActor () -> Void = {}
    }

    private var active: Entry?
    private let cancelAuthorization: @MainActor (String) -> Void

    init(cancelAuthorization: @escaping @MainActor (String) -> Void) {
        self.cancelAuthorization = cancelAuthorization
    }

    func begin(scope: BsvHomeBsvMultisigActionScope?, navigation: NavigationPathState?, onRemoval: @escaping @MainActor () -> Void) -> UUID {
        let id = UUID()
        active = Entry(id: id, scope: scope, navigation: navigation)
        active?.observerId = navigation?.observeRemoval { [weak self] in
            guard self?.contains(id) == true else { return }
            onRemoval()
        }
        return id
    }

    func contains(_ id: UUID) -> Bool {
        active?.id == id
    }

    func attach(_ cancelTask: @escaping @MainActor () -> Void, to id: UUID) {
        guard contains(id) else { cancelTask(); return }
        active?.cancelTask = cancelTask
    }

    func finish(_ id: UUID) -> Bool {
        guard let action = active, action.id == id else { return false }
        active = nil
        if let observerId = action.observerId {
            action.navigation?.removeRemovalObserver(observerId)
        }
        return true
    }

    @discardableResult
    func cancel() -> Entry? {
        guard let action = active else { return nil }
        active = nil
        action.scope?.cancel()
        action.cancelTask()
        if let scope = action.scope {
            cancelAuthorization(scope.id())
        }
        if let observerId = action.observerId {
            action.navigation?.removeRemovalObserver(observerId)
        }
        return action
    }
}
