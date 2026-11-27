// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import Foundation
import struct BsvHomeCore.BsvHomeChartHeader
import enum BsvHomeCore.BsvHomeChartValueType
import Primitives
@testable import PrimitivesComponents

public extension ChartHeaderViewModel {
    static func mock(
        period: ChartPeriod = .day,
        date: Date? = nil,
        header: BsvHomeChartHeader = .mock(),
        valueType: BsvHomeChartValueType = .price,
    ) -> ChartHeaderViewModel {
        ChartHeaderViewModel(
            period: period,
            date: date,
            header: header,
            valueType: valueType,
            formatter: CurrencyFormatter(type: .currency, currencyCode: "USD"),
        )
    }
}

public extension BsvHomeChartHeader {
    static func mock(
        value: Double = 100,
        secondaryValue: Double? = nil,
        changePercentage: Double? = 5,
    ) -> BsvHomeChartHeader {
        BsvHomeChartHeader(value: value, secondaryValue: secondaryValue, changePercentage: changePercentage)
    }
}
