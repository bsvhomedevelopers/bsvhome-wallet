// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeServiceError
import enum BsvHomeCore.BsvHomeTransferPasswordError

public extension Error {
    var isCancelled: Bool {
        switch self {
        case is CancellationError: true
        case let error as BsvHomeServiceError where error == .Cancelled: true
        case let error as BsvHomeTransferPasswordError where error == .Cancelled: true
        default: (self as NSError).code == NSURLErrorCancelled
        }
    }
}
