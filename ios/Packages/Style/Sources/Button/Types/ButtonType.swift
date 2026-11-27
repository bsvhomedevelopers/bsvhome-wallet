// Copyright (c). Gem Wallet. All rights reserved.

import Foundation

public enum ButtonType: Hashable, Sendable {
    case primary(_ buttonState: ButtonState = .normal)
    case wowPayPrimary(_ buttonState: ButtonState = .normal)

    var state: ButtonState {
        switch self {
        case let .primary(state), let .wowPayPrimary(state): state
        }
    }

    public var isDisabled: Bool {
        state != .normal
    }

    public var wowPay: Self {
        .wowPayPrimary(state)
    }
}
