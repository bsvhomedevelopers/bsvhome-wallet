// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit
import class BsvHomeCore.BsvHomeAmountService
import enum BsvHomeCore.BsvHomeStakeAmountInput
import BsvHomeCoreServicesTestKit
import Testing
@testable import Transfer

struct AmountStakeViewModelTests {
    @Test
    func title() {
        #expect(AmountStakeViewModel(asset: .mockBNB(), type: .stake(validators: [DelegationValidator.mock().map()], validator: nil), service: BsvHomeAmountService.mock()).title == "Stake")
        #expect(AmountStakeViewModel(asset: .mockBNB(), type: .unstake(delegation: Delegation.mock().map()), service: BsvHomeAmountService.mock()).title == "Unstake")
        #expect(AmountStakeViewModel(asset: .mockBNB(), type: .redelegate(validators: [DelegationValidator.mock(id: "from").map(), DelegationValidator.mock(id: "to").map()], delegation: Delegation.mock(validator: .mock(id: "from")).map(), validator: nil), service: BsvHomeAmountService.mock()).title == "Redelegate")
        #expect(AmountStakeViewModel(asset: .mockBNB(), type: .withdraw(delegation: Delegation.mock().map()), service: BsvHomeAmountService.mock()).title == "Withdraw")
        #expect(AmountStakeViewModel(asset: .mockTron(), type: .freeze(resource: Resource.bandwidth.map()), service: BsvHomeAmountService.mock()).title == "Freeze")
        #expect(AmountStakeViewModel(asset: .mockTron(), type: .unfreeze(resource: Resource.bandwidth.map()), service: BsvHomeAmountService.mock()).title == "Unfreeze")
    }

    @Test
    func validatorSelectionEnabled() {
        #expect(validatorState(.stake(validators: [DelegationValidator.mock().map()], validator: nil))?.isEnabled == true)
        #expect(validatorState(.unstake(delegation: Delegation.mock().map()))?.isEnabled == false)
        #expect(validatorState(.redelegate(validators: [DelegationValidator.mock(id: "from").map(), DelegationValidator.mock(id: "to").map()], delegation: Delegation.mock(validator: .mock(id: "from")).map(), validator: nil))?.isEnabled == true)
        #expect(validatorState(.withdraw(delegation: Delegation.mock().map()))?.isEnabled == false)
    }

    @Test
    func validatorSelection() {
        let first = DelegationValidator.mock(id: "first")
        let second = DelegationValidator.mock(id: "second")

        #expect(validatorState(.stake(validators: [first.map(), second.map()], validator: nil))?.selected.id == "first")
        #expect(validatorState(.stake(validators: [first.map(), second.map()], validator: second.map()))?.selected.id == "second")
    }

    @Test
    func resourceSelection() {
        let model = AmountStakeViewModel(asset: .mockTron(), type: .freeze(resource: Resource.energy.map()), service: BsvHomeAmountService.mock())
        guard case let .resource(state) = model.selection else {
            Issue.record("Expected resource selection")
            return
        }
        #expect(state.options == [.bandwidth, .energy])
        #expect(state.selected == .energy)
        #expect(state.isEnabled == true)
    }

    @Test
    func canChangeValue() {
        let assetData = AssetData.mock(asset: .mockBNB())
        #expect(AmountStakeViewModel(asset: .mockBNB(), type: .stake(validators: [DelegationValidator.mock().map()], validator: nil), service: BsvHomeAmountService.mock()).input(from: assetData).canChangeValue == true)
        #expect(AmountStakeViewModel(asset: .mockBNB(), type: .redelegate(validators: [DelegationValidator.mock(id: "from").map(), DelegationValidator.mock(id: "to").map()], delegation: Delegation.mock(validator: .mock(id: "from")).map(), validator: nil), service: BsvHomeAmountService.mock()).input(from: assetData).canChangeValue == true)
        #expect(AmountStakeViewModel(asset: .mockBNB(), type: .withdraw(delegation: Delegation.mock().map()), service: BsvHomeAmountService.mock()).input(from: assetData).canChangeValue == false)
        #expect(AmountStakeViewModel(asset: .mockTron(), type: .freeze(resource: Resource.bandwidth.map()), service: BsvHomeAmountService.mock()).input(from: assetData).canChangeValue == true)
        #expect(AmountStakeViewModel(asset: .mockTron(), type: .unfreeze(resource: Resource.bandwidth.map()), service: BsvHomeAmountService.mock()).input(from: assetData).canChangeValue == true)
    }

    @Test
    func availableValue() {
        let delegation = Delegation.mock(base: .mock(state: .active, balance: 5_000_000))
        let assetData = AssetData.mock(asset: .mockBNB(), balance: .mock(available: 1000))

        let stake = AmountStakeViewModel(asset: .mockBNB(), type: .stake(validators: [DelegationValidator.mock().map()], validator: nil), service: BsvHomeAmountService.mock())
        let unstake = AmountStakeViewModel(asset: .mockBNB(), type: .unstake(delegation: delegation.map()), service: BsvHomeAmountService.mock())

        #expect(stake.input(from: assetData).availableValue == 1000)
        #expect(unstake.input(from: assetData).availableValue == 5_000_000)
    }

    @Test
    func availableValueForFreezeUnfreeze() {
        let tronData = AssetData.mock(
            asset: .mockTron(),
            balance: .mock(available: 1000, frozen: 2000, locked: 3000),
        )
        let freeze = AmountStakeViewModel(asset: .mockTron(), type: .freeze(resource: Resource.bandwidth.map()), service: BsvHomeAmountService.mock())
        let unfreezeBandwidth = AmountStakeViewModel(asset: .mockTron(), type: .unfreeze(resource: Resource.bandwidth.map()), service: BsvHomeAmountService.mock())
        let unfreezeEnergy = AmountStakeViewModel(asset: .mockTron(), type: .unfreeze(resource: Resource.energy.map()), service: BsvHomeAmountService.mock())

        #expect(freeze.input(from: tronData).availableValue == 1000)
        #expect(unfreezeBandwidth.input(from: tronData).availableValue == 2000)
        #expect(unfreezeEnergy.input(from: tronData).availableValue == 3000)
    }

    @Test
    func makeTransferData() throws {
        let validator = DelegationValidator.mock(id: "validator1")
        let delegation = Delegation.mock(validator: validator)

        let stake = try AmountStakeViewModel(asset: .mockBNB(), type: .stake(validators: [validator.map()], validator: nil), service: BsvHomeAmountService.mock()).makeTransferData(value: 100, useMaxAmount: false)
        let unstake = try AmountStakeViewModel(asset: .mockBNB(), type: .unstake(delegation: delegation.map()), service: BsvHomeAmountService.mock()).makeTransferData(value: 100, useMaxAmount: false)
        let redelegate = try AmountStakeViewModel(asset: .mockBNB(), type: .redelegate(validators: [validator.map(), DelegationValidator.mock(id: "validator2").map()], delegation: delegation.map(), validator: nil), service: BsvHomeAmountService.mock()).makeTransferData(value: 100, useMaxAmount: false)
        let withdraw = try AmountStakeViewModel(asset: .mockBNB(), type: .withdraw(delegation: delegation.map()), service: BsvHomeAmountService.mock()).makeTransferData(value: 100, useMaxAmount: false)
        let freeze = try AmountStakeViewModel(asset: .mockTron(), type: .freeze(resource: Resource.bandwidth.map()), service: BsvHomeAmountService.mock()).makeTransferData(value: 100, useMaxAmount: false)
        let unfreeze = try AmountStakeViewModel(asset: .mockTron(), type: .unfreeze(resource: Resource.energy.map()), service: BsvHomeAmountService.mock()).makeTransferData(value: 100, useMaxAmount: false)

        #expect(stake.transactionType().map() == .stakeDelegate)
        #expect(unstake.transactionType().map() == .stakeUndelegate)
        #expect(redelegate.transactionType().map() == .stakeRedelegate)
        #expect(withdraw.transactionType().map() == .stakeWithdraw)
        #expect(freeze.transactionType().map() == .stakeFreeze)
        #expect(unfreeze.transactionType().map() == .stakeUnfreeze)
        #expect(stake.value == "100")
        #expect(unstake.value == "100")
        #expect(redelegate.value == "100")
        #expect(withdraw.value == "100")
        #expect(freeze.value == "100")
        #expect(unfreeze.value == "100")
    }
}

private func validatorState(_ type: BsvHomeStakeAmountInput, asset: Asset = .mockBNB()) -> SelectionState<DelegationValidator>? {
    let model = AmountStakeViewModel(asset: asset, type: type, service: BsvHomeAmountService.mock())
    if case let .validator(state) = model.selection { return state }
    return nil
}
