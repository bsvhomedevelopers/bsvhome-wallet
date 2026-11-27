// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import protocol BsvHomeCore.BsvHomeChainServiceProtocol
import BsvHomeCorePrimitives
import Primitives
import PrimitivesComponents

@Observable
@MainActor
public final class ChainListSettingsViewModel {
    private let service: any BsvHomeChainServiceProtocol

    public init(service: any BsvHomeChainServiceProtocol) {
        self.service = service
    }

    var emptyContent: EmptyContentTypeViewModel {
        EmptyContentTypeViewModel(type: .search(type: .networks))
    }

    func filterChains(for query: String) -> [Chain] {
        service.getChains(query: query).map { Chain(core: $0) }
    }
}
