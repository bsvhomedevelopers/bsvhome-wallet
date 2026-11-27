// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeNodeService
import BsvHomeCorePrimitives
import Primitives

extension BsvHomeNodeService {
    public func webSocketNode(for chain: Chain) -> URL {
        URL(string: websocketNodeUrl(chain: chain.rawValue)) ?? chain.defaultBaseUrl
    }
}
