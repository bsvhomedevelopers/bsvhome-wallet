// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeConfirmFailure
import struct BsvHomeCore.BsvHomeConfirmLoad
import enum BsvHomeCore.BsvHomeConfirmPhase
import struct BsvHomeCore.BsvHomeConfirmPreload
import struct BsvHomeCore.BsvHomeConfirmScreen
import Components
import Primitives
import PrimitivesComponents
import struct BsvHomeCore.BsvHomeSimulationWarningRow
import struct BsvHomeCore.SimulationResult
@testable import Transfer

extension ConfirmSimulationState {
    static func mock(
        result: SimulationResult? = nil,
        warnings: [BsvHomeSimulationWarningRow] = [],
        headerData: AssetValueHeaderData? = nil,
    ) -> ConfirmSimulationState {
        ConfirmSimulationState(
            result: result,
            warnings: warnings,
            hasCriticalWarning: warnings.contains { $0.severity == .critical },
            payload: SimulationPayloadModel(chain: .ethereum, primaryFields: [], secondaryFields: []),
            headerData: headerData,
            balanceChanges: [],
        )
    }
}

extension ConfirmTransferState {
    static func mock(
        load: BsvHomeConfirmLoad? = nil,
        simulation: ConfirmSimulationState = .mock(),
        feeAsset: Asset = .mock(),
        screen: BsvHomeConfirmScreen = .mock(),
    ) -> ConfirmTransferState {
        ConfirmTransferState(feeAsset: feeAsset, load: load, simulation: simulation, screen: screen)
    }
}

extension BsvHomeConfirmScreen {
    static func mock(
        phase: BsvHomeConfirmPhase = .loading,
        amountFailed: Bool = false,
        hasCriticalWarning: Bool = false,
        failure: BsvHomeConfirmFailure? = nil,
    ) -> BsvHomeConfirmScreen {
        BsvHomeConfirmScreen(phase: phase, amountFailed: amountFailed, hasCriticalWarning: hasCriticalWarning, failure: failure)
    }
}
