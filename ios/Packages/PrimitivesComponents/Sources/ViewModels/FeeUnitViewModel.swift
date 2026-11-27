// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import Foundation
import Localization
import Primitives

public struct FeeUnitViewModel {
    private let unit: FeeUnit
    private let decimals: Int
    private let symbol: String
    private let numericFormatter: NumericFormatter
    private let valueFormatter = ValueFormatter.full

    public init(
        unit: FeeUnit,
        decimals: Int,
        symbol: String,
        formatter: NumericFormatter = NumericFormatter(),
    ) {
        self.unit = unit
        self.decimals = decimals
        self.symbol = symbol
        numericFormatter = formatter
    }

    public var value: String {
        switch unit.type {
        case .satVb, .satKb, .gwei: "\(unitValueText) \(suffix)"
        case .native: unitValueText
        }
    }

    public var suffix: String {
        switch unit.type {
        case .satVb: Localized.FeeRate.satvB
        case .satKb: Localized.FeeRate.satkB
        case .gwei: Localized.FeeRate.gwei
        case .native: symbol
        }
    }

    private var unitValueText: String {
        switch unit.type {
        case .satVb, .satKb:
            valueFormatter.string(unit.value, decimals: decimals)
        case .gwei:
            numericFormatter.string(unit.value, decimals: decimals)
        case .native:
            String(
                format: "%@ %@",
                valueFormatter.string(unit.value, decimals: decimals),
                symbol,
            )
        }
    }
}
