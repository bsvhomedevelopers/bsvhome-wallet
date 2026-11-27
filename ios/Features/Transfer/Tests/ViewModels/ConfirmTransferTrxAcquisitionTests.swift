// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import struct BsvHomeCore.BsvHomeBalanceRequirement
import enum BsvHomeCore.BsvHomeConfirmError
import struct BsvHomeCore.BsvHomeConfirmInput
import struct BsvHomeCore.BsvHomeConfirmLoad
import struct BsvHomeCore.BsvHomeTransferData
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Observation
import Primitives
import PrimitivesTestKit
import Testing
@testable import Transfer

@MainActor
struct ConfirmTransferTrxAcquisitionTests {
    @Test(arguments: [
        BsvHomeConfirmError.InsufficientBalance(asset: Asset.mockTron().map(), requirement: BsvHomeBalanceRequirement(required: 2, available: 1, shortfall: 1)),
        .InsufficientNetworkFee(asset: Asset.mockTron().map(), requirement: nil),
        .InsufficientBalance(asset: Asset.mockTronUSDT().map(), requirement: BsvHomeBalanceRequirement(required: 2, available: 1, shortfall: 1)),
        .InsufficientNetworkFee(asset: Asset.mockEthereum().map(), requirement: nil),
        .Sign(error: .insufficientFunds, chain: Chain.tron.rawValue, msg: "insufficient balance"),
    ])
    func acquisitionInsufficientBalanceStaysInline(error: BsvHomeConfirmError) async {
        for load in [
            Result<BsvHomeConfirmLoad, any Error>.failure(error),
            .success(.mock(preload: .mock(amount: .error(error: error)))),
        ] {
            let model = ConfirmTransferSceneViewModel.mock(
                data: .mock(type: .transfer(.mockTronUSDT())),
                load: load,
                context: .trxAcquisition,
            )

            await model.load()

            #expect(model.state.transactionError?.displayError as? BsvHomeConfirmError == error)
            #expect(model.state.screen.action() != .execute)
            #expect(model.isPresentingSheet == nil)
            guard case let .error(_, displayError, onInfoAction) = model.itemModel(for: .error).itemModel else {
                Issue.record("Expected inline balance error")
                return
            }
            #expect(displayError as? BsvHomeConfirmError == error)
            #expect(onInfoAction == nil)

            model.onSelectListError(error: .confirm(error))

            #expect(model.isPresentingSheet == nil)
        }
    }

    @Test
    func acquisitionPreservesSafetyWarnings() {
        let model = ConfirmTransferSceneViewModel.mock(context: .trxAcquisition)

        model.onSelectListError(error: .confirm(.ScanMalicious))

        #expect(model.isPresentingSheet?.id == "info-maliciousTransaction")
    }

    @Test(arguments: [
        BsvHomeConfirmError.InsufficientBalance(asset: Asset.mockTronUSDT().map(), requirement: BsvHomeBalanceRequirement(required: 2, available: 1, shortfall: 1)),
        .InsufficientNetworkFee(asset: Asset.mockTron().map(), requirement: nil),
        .Sign(error: .insufficientFunds, chain: Chain.tron.rawValue, msg: "insufficient balance"),
    ])
    func acquisitionExecutionBalanceErrorRequiresReload(error: BsvHomeConfirmError) async {
        let session = BsvHomeConfirmSessionMock(execute: .failure(error))
        var executeCount = 0
        var completeCount = 0
        let model = ConfirmTransferSceneViewModel.mock(
            data: .mock(type: .transfer(.mockTronUSDT())),
            session: session,
            context: .trxAcquisition,
            onComplete: { completeCount += 1 },
        )
        await model.load()

        await withCheckedContinuation { failed in
            session.onExecute = {
                executeCount += 1
                withObservationTracking {
                    _ = model.state.screen.phase
                } onChange: {
                    failed.resume()
                }
            }
            model.onSelectConfirm()
        }

        #expect(executeCount == 1)
        #expect(completeCount == 0)
        #expect(model.state.transactionError?.displayError as? BsvHomeConfirmError == error)
        #expect(model.state.screen.action() == .load)
        #expect(model.isPresentingSheet == nil)
        #expect(model.isPresentingAlertMessage == nil)
        guard model.state.screen.action() == .load else { return }
        session.onExecute = { executeCount += 1 }

        await withCheckedContinuation { reloaded in
            session.onLoad = {
                withObservationTracking {
                    _ = model.state.screen.phase
                } onChange: {
                    reloaded.resume()
                }
            }
            model.onSelectConfirm()
        }

        #expect(executeCount == 1)
        #expect(completeCount == 0)
        #expect(model.state.screen.action() == .execute)
    }

    @Test(arguments: [ConfirmTransferContext.standard, .trxAcquisition])
    func executionNonBalanceErrorKeepsAlert(context: ConfirmTransferContext) async {
        let session = BsvHomeConfirmSessionMock(execute: .failure(BsvHomeConfirmError.Load(msg: "execution failure")))
        let model = ConfirmTransferSceneViewModel.mock(session: session, context: context)
        await model.load()

        await withCheckedContinuation { failed in
            session.onExecute = {
                withObservationTracking {
                    _ = model.state.screen.phase
                } onChange: {
                    failed.resume()
                }
            }
            model.onSelectConfirm()
        }

        #expect(model.state.transactionError == nil)
        #expect(model.isPresentingSheet == nil)
        #expect(model.isPresentingAlertMessage != nil)
    }

    @Test
    func ordinaryExecutionBalanceErrorKeepsAlert() async {
        let error = BsvHomeConfirmError.InsufficientNetworkFee(asset: Asset.mockTron().map(), requirement: nil)
        let session = BsvHomeConfirmSessionMock(execute: .failure(error))
        let model = ConfirmTransferSceneViewModel.mock(session: session)
        await model.load()

        await withCheckedContinuation { failed in
            session.onExecute = {
                withObservationTracking {
                    _ = model.state.screen.phase
                } onChange: {
                    failed.resume()
                }
            }
            model.onSelectConfirm()
        }

        #expect(model.state.transactionError == nil)
        #expect(model.state.screen.action() == .execute)
        #expect(model.isPresentingSheet == nil)
        #expect(model.isPresentingAlertMessage != nil)
    }

    @Test
    func tronTokenWithTrxSymbolKeepsExistingAcquisition() {
        let asset = Asset.mock(id: Asset.mockTronUSDT().id, symbol: "TRX", type: .trc20)
        let model = ConfirmTransferSceneViewModel.mock(data: .mock(type: .transfer(asset)))
        model.onSelectListError(error: .confirm(.InsufficientBalance(
            asset: asset.map(),
            requirement: BsvHomeBalanceRequirement(required: 2, available: 1, shortfall: 1),
        )))

        guard case let .info(.balanceRequired(_, _, _, .action(_, action))) = model.isPresentingSheet else {
            Issue.record("Expected token balance error")
            return
        }
        action()

        guard case let .getAsset(selectedAsset, _) = model.isPresentingSheet else {
            Issue.record("Expected existing token acquisition")
            return
        }
        #expect(selectedAsset.id == asset.id)
    }

    @Test
    func refreshPreservesTransferAndKeepsInsufficientBalanceInline() async {
        let sender = Account.mock(chain: .tron, address: "tron-sender")
        let transfer = BsvHomeTransferData.mock(type: .transfer(.mockTronUSDT()), recipient: .mock(address: "tron-recipient"), value: 20_000_000)
        let error = BsvHomeConfirmError.InsufficientNetworkFee(asset: Asset.mockTron().map(), requirement: nil)
        let session = BsvHomeConfirmSessionMock(state: .mock(preload: nil))
        var loadCount = 0
        var executeCount = 0
        session.onLoadAsync = {
            loadCount += 1
            return .mock(
                sender: sender,
                feeAsset: .mockTron(),
                preload: .mock(
                    confirmData: .mock(input: BsvHomeConfirmInput(from: sender.map(), transfer: transfer), fee: .mock(fee: BigInt(loadCount), feeAsset: "tron")),
                    amount: .error(error: error),
                ),
            )
        }
        session.onExecute = { executeCount += 1 }
        let model = ConfirmTransferSceneViewModel.mock(data: transfer, session: session)

        await model.load()
        #expect(model.isPresentingSheet?.id.hasPrefix("info-") == true)
        model.isPresentingSheet = nil

        await model.refreshAfterTrxExchange()

        #expect(loadCount == 2)
        #expect(executeCount == 0)
        #expect(model.state.confirmData?.input.transfer == transfer)
        #expect(model.state.fee?.fee == 2)
        #expect(model.state.transactionError?.displayError as? BsvHomeConfirmError == error)
        #expect(model.state.screen.action() != .execute)
        #expect(model.isPresentingSheet == nil)
        #expect(model.isPresentingAlertMessage == nil)

        model.onSelectListError(error: .confirm(error))

        guard case let .info(.insufficientNetworkFee(_, _, _, _, _, .action(_, action))) = model.isPresentingSheet else {
            Issue.record("Expected manual insufficient network fee action")
            return
        }
        action()
        #expect(model.isPresentingSheet?.id == "trx-exchange")
    }

    @Test
    func refreshCanReturnToConfirmationWithoutSending() async {
        let session = BsvHomeConfirmSessionMock()
        var executeCount = 0
        session.onExecute = { executeCount += 1 }
        let model = ConfirmTransferSceneViewModel.mock(session: session)

        await model.refreshAfterTrxExchange()

        #expect(model.state.screen.action() == .execute)
        #expect(executeCount == 0)
    }

    @Test
    func refreshDoesNotInterruptConfirmation() async {
        let session = BsvHomeConfirmSessionMock()
        var loadCount = 0
        var executeCount = 0
        session.onLoad = { loadCount += 1 }
        session.onExecute = { executeCount += 1 }
        let model = ConfirmTransferSceneViewModel.mock(session: session)
        model.state.screen = .mock(phase: .confirming)

        await model.refreshAfterTrxExchange()

        #expect(model.isConfirming)
        #expect(loadCount == 0)
        #expect(executeCount == 0)
    }

    @Test(arguments: [
        Result<BsvHomeConfirmLoad, BsvHomeConfirmError>.success(.mock(preload: .mock(
            amount: .error(error: .InsufficientNetworkFee(asset: Asset.mockTron().map(), requirement: nil)),
        ))),
        .failure(.InsufficientNetworkFee(asset: Asset.mockTron().map(), requirement: nil)),
    ])
    func oldLoadCannotOverwriteRefreshedConfirmation(response: Result<BsvHomeConfirmLoad, BsvHomeConfirmError>) async throws {
        let session = BsvHomeConfirmSessionMock(state: .mock(preload: nil))
        let model = ConfirmTransferSceneViewModel.mock(session: session)
        var reply: CheckedContinuation<BsvHomeConfirmLoad, any Error>?
        var originalTask: Task<Void, Never>?
        var refreshedTask: Task<Void, Never>?
        var loadCount = 0
        var executeCount = 0
        session.onExecute = { executeCount += 1 }

        await withCheckedContinuation { started in
            session.onLoadAsync = {
                loadCount += 1
                return try await withCheckedThrowingContinuation { continuation in
                    reply = continuation
                    started.resume()
                }
            }
            originalTask = Task { await model.load() }
        }

        session.onLoadAsync = {
            loadCount += 1
            return .mock(preload: .mock(confirmData: .mock(fee: .mock(fee: 7))))
        }
        await withCheckedContinuation { started in
            withObservationTracking {
                _ = model.state.screen.phase
            } onChange: {
                started.resume()
            }
            refreshedTask = Task { await model.refreshAfterTrxExchange() }
        }
        await Task { @MainActor in }.value
        #expect(loadCount == 1)
        #expect(model.state.screen.phase == .loading)

        let oldReply = try #require(reply)
        oldReply.resume(with: response.mapError { $0 as any Error })
        await originalTask?.value
        await refreshedTask?.value

        #expect(loadCount == 2)
        #expect(executeCount == 0)
        #expect(model.state.fee?.fee == 7)
        #expect(try await session.state().preload?.confirmData.fee.fee == 7)
        #expect(model.state.screen.action() == .execute)
        #expect(model.isPresentingSheet == nil)
    }
}
