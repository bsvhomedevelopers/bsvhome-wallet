// Copyright (c). Gem Wallet. All rights reserved.

import Components
import struct BsvHomeCore.BsvHomeSwapRate
import Localization
import PrimitivesComponents

struct TransactionRateViewModel {
    private let rate: BsvHomeSwapRate?
    private let isInverse: Bool

    init(
        rate: BsvHomeSwapRate?,
        isInverse: Bool,
    ) {
        self.rate = rate
        self.isInverse = isInverse
    }
}

extension TransactionRateViewModel: ItemModelProvidable {
    var itemModel: TransactionItemModel {
        guard let rate else { return .empty }
        return .rate(title: Localized.Buy.rate, value: AssetRateViewModel(rate: rate).text(isInverse: isInverse))
    }
}
