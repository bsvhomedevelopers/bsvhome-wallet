// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeBalanceRequirement
import enum BsvHomeCore.BsvHomeConfirmError
import struct BsvHomeCore.BsvHomeConfirmFailure
import struct BsvHomeCore.BsvHomeConfirmPreload
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import Localization
@testable import Primitives
import PrimitivesTestKit
import Testing
@testable import Transfer
import TransferTestKit

struct ConfirmErrorViewModelTests {
    @Test
    func loadFailureFillsTheList() {
        let error = BsvHomeConfirmError.Load(msg: "test error")
        let state = ConfirmTransferState.mock(screen: .mock(phase: .failed, failure: BsvHomeConfirmFailure(stage: .load, error: error)))
        let model = ConfirmErrorViewModel(error: state.transactionError, onSelectListError: { _ in })

        guard case let .error(title, errorValue, _) = model.itemModel else {
            Issue.record("Expected .error")
            return
        }
        #expect(title == Localized.Errors.errorOccurred)
        #expect(errorValue as? BsvHomeConfirmError == error)
    }

    @Test
    func executeFailureStaysOutOfTheList() {
        let failure = BsvHomeConfirmFailure(stage: .execute, error: .Broadcast(hashes: [], msg: "rejected"))
        let state = ConfirmTransferState.mock(load: .mock(preload: .mock()), screen: .mock(phase: .failed, failure: failure))
        let model = ConfirmErrorViewModel(error: state.transactionError, onSelectListError: { _ in })

        guard case .empty = model.itemModel else {
            Issue.record("Expected .empty")
            return
        }
    }

    @Test
    func transferFailure() {
        let input = BsvHomeConfirmPreload.mock(amount: .error(error: .InsufficientBalance(asset: Asset.mock().map(), requirement: BsvHomeBalanceRequirement(required: 1, available: 0, shortfall: 1))))
        let state = ConfirmTransferState.mock(load: .mock(preload: input), screen: .mock(phase: .ready, amountFailed: true))
        let model = ConfirmErrorViewModel(error: state.transactionError, onSelectListError: { _ in })

        guard case .error = model.itemModel else {
            Issue.record("Expected .error")
            return
        }
    }

    @Test
    func loaded() {
        let state = ConfirmTransferState.mock(load: .mock(preload: .mock()), screen: .mock(phase: .ready))
        let model = ConfirmErrorViewModel(error: state.transactionError, onSelectListError: { _ in })
        guard case .empty = model.itemModel else {
            Issue.record("Expected .empty")
            return
        }
    }
}
