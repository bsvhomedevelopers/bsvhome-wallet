// Copyright (c). Gem Wallet. All rights reserved.

import Foundation

@Observable
@MainActor
public final class ConfirmTransferPresentation {
    public var isPresented = false {
        didSet {
            if !isPresented {
                model?.cancelExecution()
                model = nil
            }
        }
    }

    public private(set) var model: ConfirmTransferSceneViewModel?

    public init() {}

    public func present(model: ConfirmTransferSceneViewModel) {
        self.model?.cancelExecution()
        self.model = model
        isPresented = true
    }

    public func dismiss() {
        isPresented = false
    }
}
