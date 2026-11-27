// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import Foundation
import Testing

struct PercentFormatterTests {
    let signedUS = PercentFormatter(locale: .US, sign: .signed)
    let signedUK = PercentFormatter(locale: .UK, sign: .signed)
    let unsignedUS = PercentFormatter(locale: .US, sign: .unsigned)

    @Test
    func signed() {
        #expect(signedUS.string(-1.23) == "-1.2300%")
        #expect(signedUS.string(0) == "+0.0000%")
        #expect(signedUS.string(11.12) == "+11.1200%")
        #expect(signedUS.string(11) == "+11.0000%")
        #expect(signedUS.string(12_000_123) == "+12,000,123.0000%")
        #expect(signedUS.string(1.23456789) == "+1.23456789%")

        #expect(signedUK.string(-1.23) == "-1.2300%")
        #expect(signedUK.string(11.12) == "+11.1200%")
        #expect(signedUK.string(12_000_123) == "+12,000,123.0000%")
    }

    @Test
    func unsigned() {
        #expect(unsignedUS.string(-1.23) == "1.2300%")
        #expect(unsignedUS.string(11.12) == "11.1200%")
        #expect(unsignedUS.string(11) == "11.0000%")
        #expect(unsignedUS.string(12_000_123) == "12,000,123.0000%")
        #expect(unsignedUS.string(11.1234567891234) == "11.1234567891234%")
    }
}
