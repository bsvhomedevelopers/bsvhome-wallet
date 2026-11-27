// Copyright (c). Gem Wallet. All rights reserved.

import class BsvHomeCore.BsvHomeAddressService
import BsvHomeCorePrimitives
import Primitives
import Testing

struct BsvHomeAddressServiceTests {
    @Test
    func formatsForTheChainWithTheShortStyleByDefault() {
        #expect(BsvHomeAddressService.shared.format(address: "0x12312321321312", chain: .ethereum) == "0x12312...21312")
        #expect(BsvHomeAddressService.shared.format(address: "0x12312321321312", chain: .aptos) == "0x1231...21312")
        #expect(BsvHomeAddressService.shared.format(address: "0x1231232221321312", chain: .ethereum, style: .full) == "0x1231232221321312")
    }
}
