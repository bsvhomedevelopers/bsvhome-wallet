// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
@testable import Formatters
import Foundation
import Primitives
import PrimitivesTestKit
import Testing

final class ValueFormatterTests {
    @Test
    func testShort() {
        let formatter = ValueFormatter(locale: .US, style: .short)

        #expect(formatter.string(123, decimals: 0) == "123.0000")
        #expect(formatter.string(12344, decimals: 6) == "0.012344")
        #expect(formatter.string(0, decimals: 0) == "0.0000")

        #expect(formatter.string(1_000_000, decimals: 0) == "1.0000M")
        #expect(formatter.string(1_234_567, decimals: 0) == "1.234567M")
        #expect(formatter.string(1000, decimals: 0) == "1,000.0000")
        #expect(formatter.string(100, decimals: 0) == "100.0000")
        #expect(formatter.string(10, decimals: 0) == "10.0000")
        #expect(formatter.string(1, decimals: 0) == "1.0000")

        #expect(formatter.string(1992, decimals: 4) == "0.1992")
        #expect(formatter.string(99999, decimals: 6) == "0.099999")
        #expect(formatter.string(1, decimals: 1) == "0.1000")
        #expect(formatter.string(1, decimals: 2) == "0.0100")
        #expect(formatter.string(1, decimals: 3) == "0.0010")

        #expect(formatter.string(1, decimals: 4) == "0.0001")
        #expect(formatter.string(1, decimals: 5) == "0.00001")
        #expect(formatter.string(1, decimals: 6) == "0.000001")
        #expect(formatter.string(12_345_678_910, decimals: 6) == "12,345.67891")

        #expect(formatter.string(7_758_980_129_936_940, decimals: 18, currency: "BNB") == "0.00775898012993694 BNB")
        #expect(formatter.string(2_737_071, decimals: 8, currency: "BTC") == "0.02737071 BTC")
        #expect(formatter.string(629, decimals: 8, currency: "WBTC") == "0.00000629 WBTC")
    }

    @Test
    func exactLargeAmountKeepsCompactUnits() {
        let formatter = ValueFormatter(locale: .US, style: .short)
        let amount = BigInt("123456789012345678901234567890123456789")

        #expect(formatter.string(amount, decimals: 33) == "123.456789012345678901234567890123456789K")
        #expect(formatter.string(amount, decimals: 32) == "1.23456789012345678901234567890123456789M")
        #expect(formatter.string(amount, decimals: 18) == "123,456,789,012.345678901234567890123456789B")
        #expect(formatter.string(-amount, decimals: 18) == "-123,456,789,012.345678901234567890123456789B")
        #expect(formatter.string(amount, decimals: 38) == "1.23456789012345678901234567890123456789")
    }

    @Test
    func usdtBalanceTruncatesWithoutChangingDefaultFormatting() {
        let formatter = ValueFormatter(locale: .US, style: .full)
        let usdt = Asset.mockTronUSDT()

        #expect(formatter.balanceString(123_456_789, asset: usdt) == "123.4567")
        #expect(formatter.balanceString(-123_456_789, asset: usdt) == "-123.4567")
        #expect(formatter.balanceString(1, asset: usdt) == "0.0000")
        #expect(formatter.balanceString(0, asset: usdt) == "0.0000")
        #expect(formatter.string(123_456_789, decimals: 6) == "123.456789")
    }

    @Test
    func usdtBalancesUseFourFractionDigitsAcrossNetworks() {
        let formatter = ValueFormatter(locale: .US, style: .full)
        let amount: BigInt = 123_456_789

        #expect(formatter.balanceString(amount, asset: .mockTronUSDT()) == "123.4567")
        #expect(formatter.balanceString(amount, asset: .mockEthereumUSDT(), currency: "USDT") == "123.4567 USDT")
        #expect(formatter.balanceString(amount, asset: .mockTron()) == "123.456789")
        #expect(formatter.inputString(amount, decimals: 6) == "123.456789")
    }

    @Test
    func usdtBalanceKeepsCompactUnits() {
        let formatter = ValueFormatter(locale: .US, style: .short)
        let usdt = Asset.mockTronUSDT()
        let veryLargeAmount = BigInt("123456789012345678901234567890123456789")

        #expect(formatter.balanceString(123_456_789_123, asset: usdt) == "123.4567K")
        #expect(formatter.balanceString(1_234_567_890_123, asset: usdt) == "1.2345M")
        #expect(formatter.balanceString(1_234_567_890_123_456, asset: usdt) == "1.2345B")
        #expect(formatter.balanceString(veryLargeAmount, asset: .mock(symbol: "USDT", decimals: 33)) == "123.4567K")
        #expect(formatter.balanceString(veryLargeAmount, asset: .mock(symbol: "USDT", decimals: 32)) == "1.2345M")
        #expect(formatter.balanceString(veryLargeAmount, asset: .mock(symbol: "USDT", decimals: 18)) == "123,456,789,012.3456B")
        #expect(formatter.balanceString(-veryLargeAmount, asset: .mock(symbol: "USDT", decimals: 18)) == "-123,456,789,012.3456B")
    }

    @Test
    func usdtBalanceUsesLocaleAndExactLargeAmountPath() {
        let formatter = ValueFormatter(locale: .FR, style: .full)
        let veryLargeAmount = BigInt("123456789012345678901234567890123456789")

        #expect(formatter.balanceString(123_456_789, asset: .mockTronUSDT()) == "123,4567")
        #expect(formatter.balanceString(veryLargeAmount, asset: .mock(symbol: "USDT", decimals: 36)) == "123,4567")
    }

    @Test
    func testFull() {
        let formatter = ValueFormatter(locale: .US, style: .full)

        #expect(formatter.string(123, decimals: 0) == "123.0000")
        #expect(formatter.string(12344, decimals: 6) == "0.012344")
        #expect(formatter.string(0, decimals: 0) == "0.0000")

        #expect(formatter.string(1_000_000, decimals: 0) == "1,000,000.0000")
        #expect(formatter.string(1000, decimals: 0) == "1,000.0000")
        #expect(formatter.string(100, decimals: 0) == "100.0000")
        #expect(formatter.string(10, decimals: 0) == "10.0000")
        #expect(formatter.string(1, decimals: 0) == "1.0000")

        #expect(formatter.string(1, decimals: 1) == "0.1000")
        #expect(formatter.string(1, decimals: 2) == "0.0100")
        #expect(formatter.string(1, decimals: 3) == "0.0010")
        #expect(formatter.string(1, decimals: 4) == "0.0001")
        #expect(formatter.string(1, decimals: 5) == "0.00001")
        #expect(formatter.string(1, decimals: 6) == "0.000001")
        #expect(formatter.string(BigInt("12345678910111213"), decimals: 18) == "0.012345678910111213")
        #expect(formatter.string(BigInt("1"), decimals: 18) == "0.000000000000000001")
        #expect(formatter.string(BigInt("18761627355200464162"), decimals: 18) == "18.761627355200464162")
        #expect(formatter.string(BigInt("4162"), decimals: 18) == "0.000000000000004162")
        #expect(formatter.string(1, decimals: 40) == "0.0000000000000000000000000000000000000001")
        #expect(formatter.string(BigInt("123456789012345678901234567890123456789"), decimals: 38) == "1.23456789012345678901234567890123456789")
        #expect(formatter.string(-1, decimals: 2) == "-0.0100")

        #expect(formatter.string(2_737_071, decimals: 8, currency: "BTC") == "0.02737071 BTC")
    }

    @Test
    func inputStringKeepsExistingPrecision() {
        let formatter = ValueFormatter(locale: .US, style: .full)

        #expect(formatter.inputString(0, decimals: 0) == "0")
        #expect(formatter.inputString(123, decimals: 0) == "123")
        #expect(formatter.inputString(123_456, decimals: 3) == "123.456")
        #expect(formatter.inputString(1, decimals: 8) == "0.00000001")
    }

    @Test
    func decimalConversionPreservesAtomicAmount() throws {
        let formatter = ValueFormatter(locale: .US, style: .full)

        #expect(try formatter.decimal(from: 123_456, decimals: 3) == Decimal(string: "123.456"))
        #expect(try formatter.decimal(from: -1, decimals: 2) == Decimal(string: "-0.01"))
        #expect(try formatter.decimal(from: 1, decimals: 40) == Decimal(sign: .plus, exponent: -40, significand: 1))
    }

    @Test
    func decimalConversionRejectsLostDigits() {
        let formatter = ValueFormatter(locale: .US, style: .full)

        #expect(throws: (any Error).self) {
            try formatter.decimal(from: BigInt(10).power(80) + 1, decimals: 80)
        }
        #expect(throws: (any Error).self) {
            try formatter.decimal(from: 1, decimals: 200)
        }
    }

    @Test
    func localizedMinimumFraction() {
        let formatter = ValueFormatter(locale: .FR, style: .full)

        #expect(formatter.string(123, decimals: 1) == "12,3000")
        #expect(formatter.string(123_456, decimals: 5) == "1,23456")
    }

    @Test
    func exactDigitsUseLocaleGrouping() {
        let formatter = ValueFormatter(locale: Locale(identifier: "en_IN"), style: .full)

        #expect(formatter.string(BigInt("123456789012345678901234567890123456789"), decimals: 0) == "12,34,56,78,90,12,34,56,78,90,12,34,56,78,90,12,34,56,789.0000")
    }

    @Test
    func amountToDecimal() throws {
        let formatter = ValueFormatter(locale: .US, style: .full)

        #expect(try formatter.number(amount: "123.123") == Decimal("123.123", format: .number))
        #expect(try formatter.number(amount: "0.000000000000004162") == Decimal("0.000000000000004162", format: .number))
        #expect(try formatter.number(amount: "123123495455.393686411234678911") == Decimal("123123495455.393686411234678911", format: .number))
        #expect(try formatter.number(amount: "49.393686411234678911") == Decimal("49.393686411234678911", format: .number))
        #expect(try formatter.number(amount: "49.393686762065998369") == Decimal("49.393686762065998369", format: .number))
    }

    @Test
    func fromInputUS() throws {
        let formatter = ValueFormatter(locale: .US, style: .full)

        // #expect(try formatter.inputNumber(from: "0,12317", decimals: 8) == 12317000)
        #expect(try formatter.inputNumber(from: "0.12317", decimals: 8) == 12_317_000)
        #expect(try formatter.inputNumber(from: "122,726.82", decimals: 8) == 12_272_682_000_000)
        #expect(try formatter.inputNumber(from: "122 726.82", decimals: 8) == 12_272_682_000_000)
        #expect(try formatter.inputNumber(from: "726320.82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "726,320.82083", decimals: 5) == 72_632_082_083)
        // expect(try formatter.inputNumber(from: "726'320,82083", decimals: 8))
        #expect(try formatter.inputNumber(from: "0.000000000000004162", decimals: 18) == 4162)
        #expect(try formatter.inputNumber(from: "49.393686762065998369", decimals: 18) == BigInt("49393686762065998369"))
        #expect(try formatter.inputNumber(from: "320,000", decimals: 2) == 32_000_000)
        #expect(try formatter.inputNumber(from: "320,000.00", decimals: 2) == 32_000_000)
        #expect(try formatter.inputNumber(from: "100.18054055999998", decimals: 8) == 10_018_054_055)
        // #expect(try formatter.inputNumber(from: "100,18054055999998", decimals: 8) == 10018054055)
    }

    @Test(arguments: [
        ("१२३.४५", "en_US"),
        ("১২৩.৪৫", "en_US"),
        ("๑๒๓.๔๕", "en_US"),
        ("１２３.４５", "en_US"),
        ("𝟙𝟚𝟛.𝟜𝟝", "en_US"),
        ("1२৩.4５", "en_US"),
        ("१२३.४५ BTC", "en_US"),
        ("००१२३.४५", "en_US"),
        ("१२३,४५", "fr_FR"),
    ])
    func fromUnicodeInput(input: String, localeIdentifier: String) throws {
        let formatter = ValueFormatter(locale: Locale(identifier: localeIdentifier), style: .full)

        #expect(try formatter.inputNumber(from: input, decimals: 2) == 12345)
    }

    @Test
    func fromInputRU_UA() throws {
        let formatter = ValueFormatter(locale: .UA, style: .full)

        #expect(try formatter.inputNumber(from: "0,12317", decimals: 8) == 12_317_000)
        #expect(try formatter.inputNumber(from: "0.12317", decimals: 8) == 12_317_000)
        // expect(try formatter.inputNumber(from: "122,726.82083", decimals: 8))
        #expect(try formatter.inputNumber(from: "726320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "726 320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "726'320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "320,000", decimals: 2) == 32000)
        #expect(try formatter.inputNumber(from: "320,000.00", decimals: 2) == 32000)
        #expect(try formatter.inputNumber(from: "100.18054055999998", decimals: 8) == 10_018_054_055)
        #expect(try formatter.inputNumber(from: "100,18054055999998", decimals: 8) == 10_018_054_055)
    }

    @Test
    func fromInputBR() throws {
        let formatter = ValueFormatter(locale: .PT_BR, style: .full)

        #expect(try formatter.inputNumber(from: "0,12317", decimals: 8) == 12_317_000)
        #expect(try formatter.inputNumber(from: "0.12317", decimals: 8) == 12_317_000)
        #expect(try formatter.inputNumber(from: "726320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "726 320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "726'320,82083", decimals: 8) == 72_632_082_083_000)
    }

    @Test
    func fromInputFR() throws {
        let formatter = ValueFormatter(locale: .FR, style: .full)

        #expect(try formatter.inputNumber(from: "0,12317", decimals: 8) == 12_317_000)
        #expect(try formatter.inputNumber(from: "0.12317", decimals: 8) == 12_317_000)
        #expect(try formatter.inputNumber(from: "726320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "726 320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "726'320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "110'121'212,212", decimals: 8) == 11_012_121_221_200_000)
    }

    @Test
    func fromInputDA_DK() throws {
        let formatter = ValueFormatter(locale: .DA_DK, style: .full)

        #expect(try formatter.inputNumber(from: "0,12317", decimals: 8) == 12_317_000)
        #expect(try formatter.inputNumber(from: "0.12317", decimals: 8) == 12_317_000)
        #expect(try formatter.inputNumber(from: "726320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "726 320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "726'320,82083", decimals: 8) == 72_632_082_083_000)
        #expect(try formatter.inputNumber(from: "100,18054055", decimals: 8) == 10_018_054_055)
        #expect(try formatter.inputNumber(from: "100.18054055", decimals: 8) == 10_018_054_055)
    }

    @Test
    func fromInputEN_CH() throws {
        let formatter = ValueFormatter(locale: .EN_CH, style: .full)

        #expect(try formatter.inputNumber(from: "0.005", decimals: 8) == 500_000)
        #expect(try formatter.inputNumber(from: "5.123", decimals: 8) == 512_300_000)
    }

    @Test
    func fromInputDE_CH() throws {
        let formatter = ValueFormatter(locale: .DE_CH, style: .full)

        #expect(try formatter.inputNumber(from: "0.005", decimals: 8) == 500_000)
        #expect(try formatter.inputNumber(from: "5.123", decimals: 8) == 512_300_000)
    }

    @Test
    func fromDouble() throws {
        let formatter = ValueFormatter(locale: .US, style: .full)

        #expect(try formatter.double(from: 122_131_233, decimals: 0) == Double(122_131_233.0))
    }

    @Test
    func testAuto() {
        let formatter = ValueFormatter(locale: .US, style: .auto)

        #expect(formatter.string(123, decimals: 0) == "123.0000")
        #expect(formatter.string(12344, decimals: 6) == "0.012344")
        #expect(formatter.string(11_112_344, decimals: 10) == "0.0011112344")
        #expect(formatter.string(1, decimals: 4) == "0.0001")
        #expect(formatter.string(1, decimals: 5) == "0.00001")
        #expect(formatter.string(4162, decimals: 18) == "0.000000000000004162")
        #expect(formatter.string(9_646_202_573_492, decimals: 18) == "0.000009646202573492")
        #expect(formatter.string(1, decimals: 18) == "0.000000000000000001")
    }
}
