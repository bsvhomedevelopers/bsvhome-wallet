// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeWalletConnectError
import Testing
@testable import WalletConnectorService
import WalletConnectSign

struct RejectionReasonWalletConnectorServiceTests {
    @Test
    func mapsErrors() {
        #expect(RejectionReason(from: AutoNamespacesError.requiredMethodsNotSatisfied) == .unsupportedMethods)
        #expect(RejectionReason(from: BsvHomeWalletConnectError.UnsupportedChains) == .unsupportedChains)
        #expect(RejectionReason(from: BsvHomeWalletConnectError.InvalidOrigin) == .userRejected)
    }
}
