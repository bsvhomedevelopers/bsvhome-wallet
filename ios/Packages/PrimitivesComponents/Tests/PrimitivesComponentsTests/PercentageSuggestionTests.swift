// Copyright (c). Gem Wallet. All rights reserved.

import PrimitivesComponents
import Testing

struct PercentageSuggestionTests {
    @Test
    func displayPrecisionDoesNotChangeInputValue() {
        let suggestion = PercentageSuggestion(value: 25)

        #expect(suggestion.title == "25.0000%")
        #expect(suggestion.inputValue == "25")
    }
}
