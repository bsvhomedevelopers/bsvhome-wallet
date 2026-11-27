// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import struct BsvHomeCore.BsvHomePaymentRecipient
import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit
import BsvHomeCoreServicesTestKit
import class BsvHomeCore.BsvHomeAmountService
import Testing
import class BsvHomeCore.BsvHomePerpetual
@testable import Transfer

struct AmountTransferViewModelTests {
    @Test
    func title() {
        #expect(AmountTransferViewModel(asset: .mock(), transfer: .send(payment: .mock()), service: BsvHomeAmountService.mock()).title == "Send")
        #expect(AmountTransferViewModel(asset: .mock(), transfer: .deposit, service: BsvHomeAmountService.mock()).title == "Deposit")
        #expect(AmountTransferViewModel(asset: .mock(), transfer: .withdraw, service: BsvHomeAmountService.mock()).title == "Withdraw")
    }

    @Test
    func displayAsset() {
        let usdc = Asset.mock(symbol: "USDC")

        #expect(AmountTransferViewModel(asset: usdc, transfer: .send(payment: .mock()), service: BsvHomeAmountService.mock()).displayAsset.id == usdc.id)
        #expect(AmountTransferViewModel(asset: usdc, transfer: .deposit, service: BsvHomeAmountService.mock()).displayAsset.id == usdc.id)

        let withdraw = AmountTransferViewModel(asset: usdc, transfer: .withdraw, service: BsvHomeAmountService.mock()).displayAsset
        #expect(withdraw.id.identifier == BsvHomePerpetual(provider: .hypercore).depositAsset().id)
        #expect(withdraw.type == .erc20)
    }

    @Test
    func availableValue() {
        let assetData = AssetData.mock(balance: .mock(available: 1000, withdrawable: 500))

        #expect(AmountTransferViewModel(asset: .mock(), transfer: .send(payment: .mock()), service: BsvHomeAmountService.mock()).input(from: assetData).availableValue == 1000)
        #expect(AmountTransferViewModel(asset: .mock(), transfer: .deposit, service: BsvHomeAmountService.mock()).input(from: assetData).availableValue == 1000)
        #expect(AmountTransferViewModel(asset: .mock(), transfer: .withdraw, service: BsvHomeAmountService.mock()).input(from: assetData).availableValue == 500)
    }

    @Test
    func prefilledAmount() {
        let recipient = BsvHomePaymentRecipient(recipient: .mock(address: "0x123"), amount: "1.5")
        #expect(AmountTransferViewModel(asset: .mock(), transfer: .send(payment: recipient), service: BsvHomeAmountService.mock()).prefilledAmount == "1.5")
        #expect(AmountTransferViewModel(asset: .mock(), transfer: .deposit, service: BsvHomeAmountService.mock()).prefilledAmount == nil)
    }

    @Test
    func makeTransferData() async throws {
        let send = try await AmountTransferViewModel(asset: .mock(), transfer: .send(payment: .mock()), service: BsvHomeAmountService.mock()).makeTransferData(value: 100, useMaxAmount: false)
        let deposit = try await AmountTransferViewModel(asset: .mock(), transfer: .deposit, service: BsvHomeAmountService.mock()).makeTransferData(value: 200, useMaxAmount: true)

        #expect(send.transactionType().map() == .transfer)
        #expect(deposit.transactionType().map() == .transfer)
        #expect(send.value == "100")
        #expect(deposit.value == "200")
        #expect(deposit.useMaxAmount)
    }
}
