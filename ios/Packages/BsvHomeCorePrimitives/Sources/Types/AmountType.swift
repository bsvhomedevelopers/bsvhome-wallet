// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomePaymentRecipient
import enum BsvHomeCore.BsvHomePerpetualPositionAction
import enum BsvHomeCore.BsvHomeStakeAmountInput
import Primitives

public enum AmountType: Equatable, Hashable, Sendable {
    case transfer(recipient: BsvHomePaymentRecipient)
    case deposit
    case withdraw
    case stake(BsvHomeStakeAmountInput)
    case perpetual(BsvHomePerpetualPositionAction)
    case earn(EarnType)
}
