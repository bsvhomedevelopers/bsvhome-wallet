// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import struct BsvHomeCore.BsvHomeBalanceRequirement
import enum BsvHomeCore.BsvHomeConfirmError
import BsvHomeCorePrimitives
import InfoSheet
import Primitives
import PrimitivesTestKit
import Testing
@testable import Transfer

@MainActor
struct ConfirmInfoSheetBuilderTests {
    @Test
    func insufficientBalanceSheet() {
        let asset = Asset.mock()
        let requirement = BalanceRequirement(required: 2, available: 1, shortfall: 1)

        guard case let .balanceRequired(sheetAsset, _, sheetRequirement, _) = build(for: BsvHomeConfirmError.InsufficientBalance(asset: asset.map(), requirement: BsvHomeBalanceRequirement(required: 2, available: 1, shortfall: 1))) else {
            Issue.record("Expected balanceRequired sheet")
            return
        }
        #expect(sheetAsset == asset)
        #expect(sheetRequirement == requirement)
    }

    @Test
    func minimumAccountBalanceSheet() {
        guard case let .accountMinimalBalance(_, required) = build(for: BsvHomeConfirmError.MinimumAccountBalanceTooLow(asset: Asset.mock().map(), requirement: BsvHomeBalanceRequirement(required: 10, available: 0, shortfall: 10))) else {
            Issue.record("Expected accountMinimalBalance sheet")
            return
        }
        #expect(required == BigInt(10))
    }

    @Test
    func swapBelowMinimumSheetNamesTheProvider() {
        let asset = Asset.mock()
        let error = BsvHomeConfirmError.BelowSwapMinimum(
            asset: asset.map(),
            provider: .nearIntents,
            providerName: "NEAR Intents",
            requirement: BsvHomeBalanceRequirement(required: 200, available: 150, shortfall: 50),
        )

        guard case let .swapMinimumAmount(sheetAsset, providerName, _, requirement, _, _, _) = build(for: error) else {
            Issue.record("Expected swapMinimumAmount sheet")
            return
        }
        #expect(sheetAsset == asset)
        #expect(providerName == "NEAR Intents")
        #expect(requirement == BalanceRequirement(required: 200, available: 150, shortfall: 50))
    }

    @Test
    func dustThresholdSheet() {
        let error = BsvHomeConfirmError.Sign(error: .dustThreshold, chain: Chain.bitcoin.rawValue, msg: "message can change")

        guard case let .dustThreshold(chain, _) = build(for: error) else {
            Issue.record("Expected dustThreshold sheet")
            return
        }
        #expect(chain == .bitcoin)
    }

    @Test
    func unmappedErrorsBuildNoSheet() {
        let unknown = NSError(domain: "chain", code: 1, userInfo: [NSLocalizedDescriptionKey: "connection lost"])
        let chainErrorWithoutSheet = NSError(domain: "chain", code: 1, userInfo: [NSLocalizedDescriptionKey: "insufficient balance for transfer"])

        #expect(build(for: unknown) == nil)
        #expect(build(for: chainErrorWithoutSheet) == nil)
    }

    private func build(for error: Error) -> InfoSheetType? {
        ConfirmInfoSheetBuilder.build(
            for: ConfirmTransferError(error: error),
            feePrice: nil,
            prices: [:],
            currency: Currency.usd.rawValue,
            acquireFlow: { _ in .fiat },
            networkFeeBuyAmount: 10,
            onGetAsset: { _, _ in },
        )
    }
}
