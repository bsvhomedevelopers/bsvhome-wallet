// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeCurrencyRow
import Primitives
@testable import Settings
import Testing

struct CurrencyViewModelTests {
    @Test
    func uSTitle() throws {
        let viewModel = CurrencyViewModel(row: BsvHomeCurrencyRow(currency: "USD", flag: "🇺🇸"))
        #expect(viewModel.title == "🇺🇸 USD - US Dollar")
    }

    @Test
    func eUROTitle() throws {
        let viewModel = CurrencyViewModel(row: BsvHomeCurrencyRow(currency: "EUR", flag: "🇪🇺"))
        #expect(viewModel.title == "🇪🇺 EUR - Euro")
    }
}
