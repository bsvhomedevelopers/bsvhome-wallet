// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeConfirmError
import struct BsvHomeCore.BsvHomeTransferAmount
import enum BsvHomeCore.BsvHomeTransferAmountResult
import Foundation

public typealias TransferAmountValidation = Result<BsvHomeTransferAmount, BsvHomeConfirmError>

public extension BsvHomeTransferAmountResult {
    func map() -> TransferAmountValidation {
        switch self {
        case let .amount(amount): .success(amount)
        case let .error(error): .failure(error)
        }
    }
}
