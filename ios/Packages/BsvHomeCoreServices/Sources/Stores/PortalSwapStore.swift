import protocol BsvHomeCore.BsvHomePortalSwapStore
import Store

public final class BsvHomeCorePortalSwapStore: BsvHomePortalSwapStore, @unchecked Sendable {
    private let store: PortalSwapStore

    public init(store: PortalSwapStore) {
        self.store = store
    }

    public func get(scope: String) throws -> String? {
        try store.get(scope: scope)
    }

    public func set(scope: String, value: String) throws {
        try store.set(scope: scope, value: value)
    }
}
