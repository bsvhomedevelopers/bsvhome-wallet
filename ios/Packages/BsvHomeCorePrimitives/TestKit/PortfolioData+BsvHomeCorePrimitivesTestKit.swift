// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.ChartDateValue
import enum BsvHomeCore.ChartPeriod
import struct BsvHomeCore.ChartValuePercentage
import struct BsvHomeCore.PortfolioChartData
import enum BsvHomeCore.PortfolioChartType
import struct BsvHomeCore.PortfolioData
import struct BsvHomeCore.PortfolioMarginUsage
import enum BsvHomeCore.PortfolioStatistic
import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit

public extension ChartValuePercentage {
    static func mock(
        date: Date = Date(),
        value: Float = 1500,
        percentage: Float = 5.0,
    ) -> ChartValuePercentage {
        ChartValuePercentage(date: date, value: value, percentage: percentage)
    }
}

public extension PortfolioMarginUsage {
    static func mock(
        accountValue: Double = 10000,
        usage: Double = 0.15,
    ) -> PortfolioMarginUsage {
        PortfolioMarginUsage(accountValue: accountValue, usage: usage)
    }
}

public extension PortfolioChartData {
    static func mock(
        chartType: PortfolioChartType = .value,
        values: [ChartDateValue] = Primitives.ChartDateValue.mockHistory().map { $0.map() },
    ) -> PortfolioChartData {
        PortfolioChartData(chartType: chartType, values: values)
    }
}

public extension PortfolioData {
    static func mockWallet(
        charts: [PortfolioChartData] = [.mock(chartType: .value)],
        statistics: [PortfolioStatistic] = [
            .allTimeHigh(value: .mock()),
            .allTimeLow(value: .mock(value: 800, percentage: -10.0)),
        ],
        availablePeriods: [ChartPeriod] = [.day, .week, .month, .year, .all],
    ) -> PortfolioData {
        PortfolioData(charts: charts, statistics: statistics, availablePeriods: availablePeriods)
    }

    static func mockPerpetual(
        charts: [PortfolioChartData] = [
            .mock(chartType: .pnl, values: Primitives.ChartDateValue.mockHistory(values: [0, 5, 2, 8, 10]).map { $0.map() }),
            .mock(chartType: .value, values: Primitives.ChartDateValue.mockHistory(values: [100, 105, 102, 108, 110]).map { $0.map() }),
        ],
        statistics: [PortfolioStatistic] = [
            .unrealizedPnl(value: 500),
            .accountLeverage(value: 2.5),
            .marginUsage(value: .mock()),
            .allTimePnl(value: 1200),
            .volume(value: 50000),
        ],
        availablePeriods: [ChartPeriod] = [.day, .week, .month, .year, .all],
    ) -> PortfolioData {
        PortfolioData(charts: charts, statistics: statistics, availablePeriods: availablePeriods)
    }
}
