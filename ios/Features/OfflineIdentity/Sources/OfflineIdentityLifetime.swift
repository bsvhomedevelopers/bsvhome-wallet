import Foundation

@MainActor
final class OfflineIdentityLifetime {
    let model: OfflineIdentityViewModel

    init(model: OfflineIdentityViewModel) {
        self.model = model
    }

    deinit {
        let model = model
        Task { @MainActor in model.clearSensitiveState() }
    }
}
