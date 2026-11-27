// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Primitives
import PrimitivesComponents
import Testing

struct NetworkSelectorViewModelTests {
    private let model = NetworkSelectorViewModel(
        state: .data(.plain([.bitcoin, .ethereum])),
        selectedItems: [],
        selectionType: .checkmark,
        environment: .production,
    )

    @Test
    func searchable() {
        #expect(model.search != nil)
        #expect(model.items == [.bitcoin, .ethereum])
    }

    @Test
    func productScopeHidesUnsupportedItemsAndRetainedSelections() {
        let model = NetworkSelectorViewModel(
            state: .data(.section([
                ListSection(id: "hidden", title: nil, image: nil, values: [.ethereum, .tronShasta]),
                ListSection(id: "supported", title: nil, image: nil, values: [.bitcoinSv, .tron, .bitcoin]),
            ])),
            selectedItems: [.ethereum, .tronShasta, .tron, .bitcoinSv],
            environment: .wowPay,
        )
        #expect(model.items == [.bitcoinSv])
        #expect(model.selectedItems == [.bitcoinSv])
        guard let value = model.state.value, case let .section(sections) = value else {
            Issue.record("Expected grouped network list")
            return
        }
        #expect(sections.map(\.id) == ["supported"])
    }

    @Test
    func filtersByQuery() throws {
        let search = try #require(model.search)

        #expect(search.filter(.bitcoin, "bitc"))
        #expect(!search.filter(.bitcoin, "ethereum"))
    }
}
