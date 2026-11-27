// Copyright (c). Gem Wallet. All rights reserved.

import protocol BsvHomeCore.BsvHomeConfirmSessionProtocol

final class ConfirmExecutionLifetime {
    private let session: any BsvHomeConfirmSessionProtocol
    private weak var model: ConfirmTransferSceneViewModel?

    init(session: any BsvHomeConfirmSessionProtocol, model: ConfirmTransferSceneViewModel) {
        self.session = session
        self.model = model
    }

    deinit {
        session.cancel()
        Task { @MainActor [weak model] in model?.cancelExecution() }
    }
}
