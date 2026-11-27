// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import Primitives
import WalletConnector
import struct BsvHomeCore.BsvHomeTransferData
import struct BsvHomeCore.SimulationResult

public struct ConfirmTransferRequest: Sendable {
    public let data: BsvHomeTransferData
    public let simulation: SimulationResult?
    public let delegate: TransferDataCallback.ConfirmTransferDelegate?

    public init(
        data: BsvHomeTransferData,
        simulation: SimulationResult? = nil,
        delegate: TransferDataCallback.ConfirmTransferDelegate? = nil,
    ) {
        self.data = data
        self.simulation = simulation
        self.delegate = delegate
    }
}
