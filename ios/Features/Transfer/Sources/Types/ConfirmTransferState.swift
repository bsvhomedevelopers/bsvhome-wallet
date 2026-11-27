// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeConfirmData
import enum BsvHomeCore.BsvHomeConfirmError
import enum BsvHomeCore.BsvHomeConfirmFeeSelection
import struct BsvHomeCore.BsvHomeConfirmLoad
import struct BsvHomeCore.BsvHomeConfirmMetadata
import struct BsvHomeCore.BsvHomeConfirmPreload
import struct BsvHomeCore.BsvHomeConfirmScreen
import struct BsvHomeCore.BsvHomeFeeAsset
import struct BsvHomeCore.BsvHomeFeeRateRows
import struct BsvHomeCore.BsvHomeTransactionLoadFee
import struct BsvHomeCore.BsvHomeTransferData
import Components
import Foundation
import Primitives
import PrimitivesComponents

struct ConfirmTransferState {
    var feeAsset: Asset
    var load: BsvHomeConfirmLoad?
    var simulation: ConfirmSimulationState
    var screen: BsvHomeConfirmScreen

    var metadata: BsvHomeConfirmMetadata? { load?.metadata }
    var feeAssets: [BsvHomeFeeAsset] { load?.feeAssets ?? [] }
    var confirmData: BsvHomeConfirmData? { load?.preload?.confirmData }
    var addressName: AddressName? { load?.addressName.map { $0.map() } }
}

extension ConfirmTransferState {
    init(transfer: BsvHomeTransferData, simulation: ConfirmSimulationState, screen: BsvHomeConfirmScreen) {
        self.init(
            feeAsset: transfer.feeAsset().map(),
            load: nil,
            simulation: simulation,
            screen: screen,
        )
    }

    init(_ load: BsvHomeConfirmLoad, screen: BsvHomeConfirmScreen) throws {
        self.init(
            feeAsset: load.feeAsset.map(),
            load: load,
            simulation: try ConfirmSimulationState(load.simulation),
            screen: screen,
        )
    }

    var preload: BsvHomeConfirmPreload? {
        load?.preload
    }

    var transferAmount: TransferAmountValidation? {
        preload?.amount.map()
    }

    var fee: BsvHomeTransactionLoadFee? {
        preload?.confirmData.fee
    }

    func feeRateRows(selection: BsvHomeConfirmFeeSelection) -> BsvHomeFeeRateRows? {
        confirmData?.feeRateRows(selection: selection, feeAsset: feeAsset.map())
    }

    var transactionError: ConfirmTransferError? {
        if let failure = screen.failure, failure.stage == .load { return ConfirmTransferError(error: failure.error) }
        if case let .failure(error)? = transferAmount { return ConfirmTransferError(error: error) }
        return nil
    }
}

extension Error {
    var confirmError: BsvHomeConfirmError {
        self as? BsvHomeConfirmError ?? .Load(msg: localizedDescription)
    }
}
