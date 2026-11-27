// Copyright (c). Gem Wallet. All rights reserved.

import Formatters

public struct PercentageSuggestion: SuggestionViewable {
    public let id: Int
    public let value: Int

    public var title: String {
        PercentFormatter.unsigned.string(Double(value))
    }

    public var inputValue: String {
        String(value)
    }

    public init(value: Int) {
        id = value
        self.value = value
    }
}
