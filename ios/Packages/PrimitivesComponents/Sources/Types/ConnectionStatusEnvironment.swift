// Copyright (c). Gem Wallet. All rights reserved.

import class BsvHomeCore.BsvHomeConnectionService
import enum BsvHomeCore.BsvHomeRefreshKind
import BsvHomeCorePrimitives
import Primitives
import SwiftUI

public extension EnvironmentValues {
    @Entry var connectionStatus: ConnectionStatus = .online
}

public extension ConnectionStatus {
    func refreshInterval(for kind: BsvHomeRefreshKind) -> TimeInterval {
        BsvHomeConnectionService.shared.refreshInterval(kind: kind, status: map())
    }
}
