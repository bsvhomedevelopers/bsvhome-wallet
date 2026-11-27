// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import PrimitivesComponents

public struct SlippageSuggestion: SuggestionViewable {
    public let id: UInt32
    private let percentText: String

    public var title: String {
        PercentFormatter.unsigned.string(Double(id) / 100)
    }

    public var inputValue: String {
        percentText
    }

    public init(bps: UInt32, percentText: String) {
        id = bps
        self.percentText = percentText
    }
}
