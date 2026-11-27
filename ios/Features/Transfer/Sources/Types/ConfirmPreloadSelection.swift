// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeConfirmFeeSelection
import Primitives

struct ConfirmPreloadSelection: Equatable {
    let fee: BsvHomeConfirmFeeSelection
    let feeAsset: FeeAssetSelection
}
