// Copyright (c). Gem Wallet. All rights reserved.

public import BigInt
public import enum BsvHomeCore.FeePriority
public import enum BsvHomeCore.GasPriceType
public import struct BsvHomeCore.BsvHomeAssetBalance
public import struct BsvHomeCore.BsvHomeConfirmData
public import struct BsvHomeCore.BsvHomeConfirmInput
public import struct BsvHomeCore.BsvHomeConfirmLoad
public import struct BsvHomeCore.BsvHomeConfirmMetadata
public import struct BsvHomeCore.BsvHomeConfirmPreload
public import struct BsvHomeCore.BsvHomeConfirmSimulation
public import struct BsvHomeCore.BsvHomeConfirmSimulationState
public import struct BsvHomeCore.BsvHomeFeeAsset
public import struct BsvHomeCore.BsvHomeFeeOptions
public import struct BsvHomeCore.BsvHomeFeeRate
public import struct BsvHomeCore.BsvHomeSimulationWarningRow
public import struct BsvHomeCore.BsvHomeTransactionLoadFee
public import enum BsvHomeCore.BsvHomeTransactionLoadMetadata
public import struct BsvHomeCore.BsvHomeTransferAmount
public import enum BsvHomeCore.BsvHomeTransferAmountResult
public import struct BsvHomeCore.SimulationResult
public import struct BsvHomeCore.TransferDataExtra
import Foundation
import struct BsvHomeCore.ApprovalData
import struct BsvHomeCore.BsvHomeTransferData
import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit

public extension BsvHomeConfirmData {
    static func mock(
        input: BsvHomeConfirmInput = BsvHomeConfirmInput(from: Primitives.Account.mock().map(), transfer: BsvHomeTransferData.mock()),
        fee: BsvHomeTransactionLoadFee = .mock(),
        selectedPriority: BsvHomeCore.FeePriority = .normal,
        feeRates: [BsvHomeFeeRate] = [],
        metadata: BsvHomeTransactionLoadMetadata = .none,
        simulation: SimulationResult? = .none,
    ) -> BsvHomeConfirmData {
        BsvHomeConfirmData(
            input: input,
            fee: fee,
            selectedPriority: selectedPriority,
            feeRates: feeRates,
            metadata: metadata,
            simulation: simulation,
        )
    }
}

public extension BsvHomeTransactionLoadFee {
    static func mock(
        fee: BigInt = 1,
        gasPriceType: GasPriceType = .regular(gasPrice: 1),
        gasLimit: BigInt = 1,
        options: BsvHomeFeeOptions = BsvHomeFeeOptions(options: [:]),
        feeAsset: String = "bitcoin",
    ) -> BsvHomeTransactionLoadFee {
        BsvHomeTransactionLoadFee(
            fee: fee,
            gasPriceType: gasPriceType,
            gasLimit: gasLimit,
            options: options,
            feeAsset: feeAsset,
        )
    }
}

public extension BsvHomeConfirmPreload {
    static func mock(
        confirmData: BsvHomeConfirmData = .mock(),
        amount: BsvHomeTransferAmountResult = .amount(amount: BsvHomeTransferAmount(value: 1, networkFee: 1, isMaxAmount: false)),
    ) -> BsvHomeConfirmPreload {
        BsvHomeConfirmPreload(confirmData: confirmData, amount: amount)
    }
}

public extension BsvHomeAssetBalance {
    static func mock(assetId: String) -> BsvHomeAssetBalance {
        BsvHomeAssetBalance(
            assetId: assetId,
            available: 0,
            frozen: 0,
            locked: 0,
            staked: 0,
            pending: 0,
            pendingUnconfirmed: 0,
            rewards: 0,
            reserved: 0,
            withdrawable: 0,
            earn: 0,
            metadata: nil,
            isActive: true,
        )
    }
}

public extension BsvHomeConfirmMetadata {
    static func mock(assetId: String = Primitives.Asset.mock().id.identifier) -> BsvHomeConfirmMetadata {
        BsvHomeConfirmMetadata(
            assetBalance: .mock(assetId: assetId),
            feeAssetBalance: .mock(assetId: assetId),
            prices: [],
        )
    }
}

public extension TransferDataExtra {
    static func mock(
        to: String = "",
        gasLimit: BigInt? = .none,
        gasPrice: GasPriceType? = .none,
        data: Data? = .none,
        outputType: Primitives.TransferDataOutputType = .encodedTransaction,
        outputAction: Primitives.TransferDataOutputAction = .send,
        transactionType: Primitives.TransactionType = .transfer,
        approval: BsvHomeCore.ApprovalData? = .none,
    ) -> TransferDataExtra {
        TransferDataExtra(
            to: to,
            gasLimit: gasLimit,
            gasPrice: gasPrice,
            data: data,
            outputType: outputType.map(),
            outputAction: outputAction.map(),
            transactionType: transactionType.map(),
            approval: approval,
        )
    }
}

public extension BsvHomeTransferAmount {
    static func mock(
        value: BigInt = 100,
        networkFee: BigInt = 1,
        isMaxAmount: Bool = false,
    ) -> BsvHomeTransferAmount {
        BsvHomeTransferAmount(value: value, networkFee: networkFee, isMaxAmount: isMaxAmount)
    }
}

public extension BsvHomeConfirmLoad {
    static func mock(
        sender: Primitives.Account = .mock(),
        feeAsset: Primitives.Asset = .mockEthereum(),
        metadata: BsvHomeConfirmMetadata = .mock(),
        feeAssets: [BsvHomeFeeAsset] = [],
        simulation: BsvHomeConfirmSimulation? = nil,
        warnings: [BsvHomeSimulationWarningRow] = [],
        addressName: Primitives.AddressName? = nil,
        preload: BsvHomeConfirmPreload? = .mock(),
    ) -> BsvHomeConfirmLoad {
        BsvHomeConfirmLoad(
            sender: sender.map(),
            feeAsset: feeAsset.map(),
            metadata: metadata,
            feeAssets: feeAssets,
            simulation: BsvHomeConfirmSimulationState(chain: Primitives.Chain.ethereum.rawValue, result: nil, warnings: warnings, simulation: simulation, addressNames: []),
            addressName: addressName?.map(),
            preload: preload,
        )
    }
}
