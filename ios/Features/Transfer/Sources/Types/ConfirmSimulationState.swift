// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeConfirmSimulation
import struct BsvHomeCore.BsvHomeConfirmSimulationState
import struct BsvHomeCore.BsvHomeSimulationBalanceChange
import BsvHomeCorePrimitives
import Primitives
import PrimitivesComponents
import struct BsvHomeCore.BsvHomeSimulationWarningRow
import struct BsvHomeCore.SimulationResult
import func BsvHomeCore.simulationWarningRows

struct ConfirmSimulationState {
    let result: SimulationResult?
    let warnings: [BsvHomeSimulationWarningRow]
    let hasCriticalWarning: Bool
    let payload: SimulationPayloadModel
    let headerData: AssetValueHeaderData?
    let balanceChanges: [BsvHomeSimulationBalanceChange]

    init(
        result: SimulationResult?,
        warnings: [BsvHomeSimulationWarningRow],
        hasCriticalWarning: Bool,
        payload: SimulationPayloadModel,
        headerData: AssetValueHeaderData?,
        balanceChanges: [BsvHomeSimulationBalanceChange],
    ) {
        self.result = result
        self.warnings = warnings
        self.hasCriticalWarning = hasCriticalWarning
        self.payload = payload
        self.headerData = headerData
        self.balanceChanges = balanceChanges
    }

    init(result: SimulationResult?, chain: Primitives.Chain) {
        self.init(
            result: result,
            warnings: simulationWarningRows(warnings: result?.warnings ?? []),
            hasCriticalWarning: false,
            payload: SimulationPayloadModel(chain: chain, primaryFields: [], secondaryFields: []),
            headerData: nil,
            balanceChanges: [],
        )
    }

    init(_ state: BsvHomeConfirmSimulationState) throws {
        let details = state.simulation
        let simulation = state.result
        var payload = SimulationPayloadModel(
            chain: Primitives.Chain(core: state.chain),
            primaryFields: details?.primaryFields ?? [],
            secondaryFields: details?.secondaryFields ?? [],
        )
        payload.addressNames = state.names
        self.init(
            result: simulation,
            warnings: state.warnings,
            hasCriticalWarning: details?.hasCriticalWarning ?? false,
            payload: payload,
            headerData: details?.header?.map(),
            balanceChanges: details?.balanceChanges ?? [],
        )
    }
}
