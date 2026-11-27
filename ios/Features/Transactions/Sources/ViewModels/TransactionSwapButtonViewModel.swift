// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import struct BsvHomeCore.BsvHomeSwapAgain
import Localization
import Primitives

public struct TransactionSwapButtonViewModel {
    private let swapAgain: BsvHomeSwapAgain?

    public init(swapAgain: BsvHomeSwapAgain?) {
        self.swapAgain = swapAgain
    }
}

extension TransactionSwapButtonViewModel: ItemModelProvidable {
    public var itemModel: TransactionItemModel {
        guard swapAgain != nil else {
            return .empty
        }
        return TransactionItemModel.swapAgain(text: Localized.Transaction.swapAgain)
    }
}
