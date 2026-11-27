// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import enum BsvHomeCore.BsvHomeAmountSign
import Primitives
@testable import PrimitivesComponents

public extension AmountDisplayStyle {
    static func mock(
        sign: BsvHomeAmountSign = .none,
        formatter: ValueFormatter = .full,
        currencyCode: String = "USD",
    ) -> AmountDisplayStyle {
        AmountDisplayStyle(
            sign: sign,
            formatter: formatter,
            currencyCode: currencyCode,
        )
    }
}
