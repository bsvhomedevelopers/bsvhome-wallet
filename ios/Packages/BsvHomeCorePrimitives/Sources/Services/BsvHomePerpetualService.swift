// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomePerpetualServiceProtocol
import Primitives

public extension BsvHomePerpetualServiceProtocol {
    func setPinned(_ isPinned: Bool, perpetualId: PerpetualId) async throws {
        try await setPinned(perpetualId: perpetualId.identifier, pinned: isPinned)
    }
}
