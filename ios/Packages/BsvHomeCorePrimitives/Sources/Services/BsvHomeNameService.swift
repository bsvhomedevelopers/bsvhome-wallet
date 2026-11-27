// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeNameRecordState
import protocol BsvHomeCore.BsvHomeNameServiceProtocol
import Primitives

public extension BsvHomeNameServiceProtocol {
    func getNameRecord(name: String, chain: Chain) async throws -> BsvHomeNameRecordState {
        try await getNameRecord(name: name, chain: chain.rawValue)
    }
}
