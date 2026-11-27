// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import BsvHomeCorePrimitivesTestKit
import Primitives
import PrimitivesTestKit
import Transfer
import WalletConnector
import struct BsvHomeCore.BsvHomeTransferData
import struct BsvHomeCore.SimulationResult

public extension ConfirmTransferRequest {
    static func mock(
        data: BsvHomeTransferData = .mock(),
        simulation: SimulationResult? = nil,
        delegate: TransferDataCallback.ConfirmTransferDelegate? = nil,
    ) -> ConfirmTransferRequest {
        ConfirmTransferRequest(data: data, simulation: simulation, delegate: delegate)
    }
}
