// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.SignMessage
import Primitives
import BsvHomeCorePrimitivesTestKit
import PrimitivesTestKit
import WalletConnectorService
import struct BsvHomeCore.SimulationResult

public extension SignMessagePayload {
    static func mock(
        chain: Chain = .ethereum,
        session: WalletConnectionSession = .mock(),
        wallet: Wallet = .mock(),
        message: SignMessage = .mock(),
        simulation: SimulationResult = .mock(),
        assets: [Asset] = [],
    ) -> SignMessagePayload {
        SignMessagePayload(
            chain: chain,
            session: session,
            wallet: wallet,
            message: message,
            simulation: simulation,
            assets: assets,
        )
    }
}
