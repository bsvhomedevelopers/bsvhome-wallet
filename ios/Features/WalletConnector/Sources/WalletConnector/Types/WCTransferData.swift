// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeWalletConnectTransactionRequest
import BsvHomeCorePrimitives
import Primitives
import struct BsvHomeCore.BsvHomeTransferData
import struct BsvHomeCore.SimulationResult

public struct WCTransferData: Identifiable, Sendable {
    public let transferData: BsvHomeTransferData
    public let wallet: Wallet
    public let simulation: SimulationResult

    public init(transferData: BsvHomeTransferData, wallet: Wallet, simulation: SimulationResult) {
        self.transferData = transferData
        self.wallet = wallet
        self.simulation = simulation
    }

    public init(_ request: BsvHomeWalletConnectTransactionRequest) {
        self.init(
            transferData: request.transfer,
            wallet: request.wallet.map(),
            simulation: request.simulation,
        )
    }

    public var id: String {
        wallet.id.id
    }
}
