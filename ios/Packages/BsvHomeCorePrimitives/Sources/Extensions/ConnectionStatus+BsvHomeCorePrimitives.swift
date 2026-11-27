// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import func BsvHomeCore.connectionStatus
import Primitives

public extension [Primitives.ConnectionComponent] {
    var connectionStatus: Primitives.ConnectionStatus {
        BsvHomeCore.connectionStatus(unhealthyComponents: map { $0.map() }).map()
    }
}
