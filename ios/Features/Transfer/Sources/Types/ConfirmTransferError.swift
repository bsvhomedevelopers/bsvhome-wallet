// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeConfirmError
import Primitives

enum ConfirmTransferError {
    case confirm(BsvHomeConfirmError)
    case other(Error)

    init(error: Error) {
        switch error {
        case let error as BsvHomeConfirmError where error.hasInfoSheet:
            self = .confirm(error)
        default:
            self = .other(error)
        }
    }

    var displayError: Error {
        switch self {
        case let .confirm(error): error
        case let .other(error): error
        }
    }

    var isInsufficientBalance: Bool {
        guard let error = displayError as? BsvHomeConfirmError else { return false }
        return switch error {
        case .InsufficientBalance, .InsufficientNetworkFee, .Sign(.insufficientFunds, _, _): true
        default: false
        }
    }
}
