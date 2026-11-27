// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeSearchScope
import protocol BsvHomeCore.BsvHomeAssetSelectionServiceProtocol
import Primitives

public extension BsvHomeAssetSelectionServiceProtocol {
    @discardableResult
    func search(query: String, scope: WalletSearchTag) async throws -> Bool {
        try await search(query: query, scope: scope.bsvHomeScope)
    }
}

public extension WalletSearchTag {
    var bsvHomeScope: BsvHomeSearchScope {
        switch self {
        case .all: .all
        case let .list(id): .list(id: id)
        }
    }
}
