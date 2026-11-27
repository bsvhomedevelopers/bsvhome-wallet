// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import Foundation
import struct BsvHomeCore.ChartDateValue
import struct BsvHomeCore.BsvHomeChartData
import struct BsvHomeCore.BsvHomeChartHeader
import enum BsvHomeCore.BsvHomeChartValueType
import Primitives
@testable import PrimitivesComponents
import PrimitivesTestKit
import SwiftUI

public extension ChartValuesViewModel {
    static func mock(
        period: ChartPeriod = .day,
        chartData: BsvHomeChartData = .mock(),
    ) -> ChartValuesViewModel {
        ChartValuesViewModel(
            period: period,
            chartData: chartData,
            formatter: CurrencyFormatter(type: .currency, currencyCode: "USD"),
        )!
    }
}

public extension BsvHomeChartData {
    static func mock(
        values: [Double] = [100, 150, 80, 120],
        header: BsvHomeChartHeader? = .mock(),
        valueType: BsvHomeChartValueType = .price,
    ) -> BsvHomeChartData {
        BsvHomeChartData(
            valueType: valueType,
            base: values.first ?? 0,
            showsSecondaryValue: false,
            values: values.enumerated().map {
                BsvHomeCore.ChartDateValue(date: Date(timeIntervalSince1970: Double($0.offset) * 3600), value: $0.element)
            },
            header: header,
        )
    }
}
