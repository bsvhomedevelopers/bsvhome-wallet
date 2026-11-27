// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeAutocloseSummary
import Components
import Formatters
import Foundation
import BsvHomeCorePrimitives
import Localization
import Primitives

public struct PerpetualModifyViewModel: Sendable {
    private let summary: BsvHomeAutocloseSummary?
    private let autocloseFormatter = AutocloseFormatter(
        takeProfitLabel: Localized.Perpetual.takeProfit,
        stopLossLabel: Localized.Perpetual.stopLoss,
    )

    public init(summary: BsvHomeAutocloseSummary?) {
        self.summary = summary
    }

    public var listItemModel: ListItemModel? {
        guard let summary else {
            return nil
        }
        let autoclose = autocloseFormatter.format(
            takeProfit: summary.takeProfit,
            stopLoss: summary.stopLoss,
            takeProfitCanceled: summary.takeProfitCleared,
            stopLossCanceled: summary.stopLossCleared,
        )

        return ListItemModel(
            title: Localized.Perpetual.autoClose,
            subtitle: autoclose.subtitle,
            subtitleExtra: autoclose.subtitleExtra,
        )
    }
}
