@testable import BsvColdWallet
import Foundation
import Testing

struct BsvColdWalletAmountTests {
    @Test
    func exactAmountsRoundTrip() {
        #expect(BsvColdWalletAmount.satoshis("0.00001") == 1000)
        #expect(BsvColdWalletAmount.satoshis("1.00000001") == 100_000_001)
        #expect(BsvColdWalletAmount.satoshis("0,00001", decimalSeparator: ",") == 1000)
        #expect(BsvColdWalletAmount.display(1) == "0.00000001")
        #expect(BsvColdWalletAmount.display(1000) == "0.00001")
        #expect(BsvColdWalletAmount.display(100_000_000) == "1")
    }

    @Test(arguments: ["", "0", "-1", "+1", "1e-5", "0.000000001", "184467440738", "1,000", "1.", ".1", " 1", "١"])
    func invalidAmountsCannotPrepare(input: String) {
        #expect(BsvColdWalletAmount.satoshis(input) == nil)
    }

    @Test(arguments: ["de_DE", "fr_FR"])
    func localizedAmountsRejectGroupingAndMixedSeparators(identifier: String) {
        let separator = Locale(identifier: identifier).decimalSeparator ?? "."
        #expect(BsvColdWalletAmount.satoshis("1,234", decimalSeparator: separator) == 123_400_000)
        #expect(BsvColdWalletAmount.satoshis("1.234", decimalSeparator: separator) == nil)
        #expect(BsvColdWalletAmount.satoshis("1.234,56", decimalSeparator: separator) == nil)
        #expect(BsvColdWalletAmount.satoshis("1 234,56", decimalSeparator: separator) == nil)
        #expect(BsvColdWalletAmount.satoshis("1\u{202f}234,56", decimalSeparator: separator) == nil)
    }
}
