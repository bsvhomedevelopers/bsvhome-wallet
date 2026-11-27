import Foundation

@MainActor
final class BsvColdWalletLifetime {
    let model: BsvColdWalletViewModel

    init(model: BsvColdWalletViewModel) {
        self.model = model
    }

    deinit {
        let model = model
        Task { @MainActor in model.disappear() }
    }
}
