// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeConfirmData
import struct BsvHomeCore.BsvHomeConfirmLoad
import enum BsvHomeCore.BsvHomeExecuteResult
import struct BsvHomeCore.BsvHomeTransferData
import struct BsvHomeCore.SimulationResult
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesComponents
import StoreTestKit
@testable import Transfer
import TransferTestKit

@MainActor
extension ConfirmTransferSceneViewModel {
    static func mock(
        request: ConfirmTransferRequest? = nil,
        wallet: Wallet? = nil,
        data: BsvHomeTransferData = .mock(),
        simulation: SimulationResult? = nil,
        bsvHomeConfirmService: BsvHomeConfirmServiceMock = BsvHomeConfirmServiceMock(),
        load: Result<BsvHomeConfirmLoad, any Error> = .success(.mock()),
        execute: Result<BsvHomeExecuteResult, any Error> = .success(.signed(data: [])),
        session: BsvHomeConfirmSessionMock? = nil,
        context: ConfirmTransferContext = .standard,
        onComplete: VoidAction = nil,
    ) -> ConfirmTransferSceneViewModel {
        let wallet = wallet ?? .mock(accounts: [.mock(chain: data.chain)])
        return ConfirmTransferSceneViewModel(
            request: request ?? ConfirmTransferRequest(data: data, simulation: simulation),
            wallet: wallet,
            session: session ?? BsvHomeConfirmSessionMock(
                state: .mock(feeAsset: data.feeAsset().map(), simulation: bsvHomeConfirmService.simulation, preload: nil),
                load: load,
                execute: execute,
            ),
            context: context,
            onComplete: onComplete,
        )
    }
}
