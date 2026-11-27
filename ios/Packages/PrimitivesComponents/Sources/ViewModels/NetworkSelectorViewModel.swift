// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import class BsvHomeCore.BsvHomeChainService
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import Localization
import Primitives
import Style
import SwiftUI

public struct NetworkSelectorViewModel: SelectableSheetViewable {
    public var selectionType: SelectionType

    private let chainService: BsvHomeChainService

    public let state: StateViewType<SelectableListType<Chain>>

    public var selectedItems: Set<Chain>
    public private(set) var search: ListSearch<Chain>?

    public let title: String

    public init(
        state: StateViewType<SelectableListType<Chain>>,
        selectedItems: [Chain] = [],
        selectionType: SelectionType = .navigationLink,
        title: String = Localized.Settings.Networks.title,
        environment: BsvHomeRuntimeEnvironment = .current,
    ) {
        chainService = BsvHomeChainService.newWithEnvironment(environment: environment)
        self.selectionType = selectionType
        let available = Set(environment.availableChains())
        self.state = state.map { list in
            switch list {
            case let .plain(chains):
                .plain(chains.filter { available.contains($0.rawValue) })
            case let .section(sections):
                .section(sections.compactMap { section in
                    let values = section.values.filter { available.contains($0.rawValue) }
                    return values.isEmpty ? nil : ListSection(id: section.id, title: section.title, image: section.image, values: values)
                })
            }
        }
        self.selectedItems = Set(selectedItems.filter { available.contains($0.rawValue) })
        self.title = title
        search = ListSearch(
            filter: filter(chain:query:),
            emptyContent: EmptyContentTypeViewModel(type: .search(type: EmptyContentType.SearchType.networks)),
        )
    }

    public var cancelButtonTitle: String { Localized.Common.cancel }
    public var clearButtonTitle: String { Localized.Filter.clear }
    public var doneButtonTitle: String { Localized.Common.done }
    public var confirmButtonTitle: String { Localized.Transfer.confirm }

    private func filter(chain: Chain, query: String) -> Bool {
        !chainService.getMatchingChains(chains: [chain.rawValue], query: query).isEmpty
    }
}
