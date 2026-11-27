// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeConfirmError
import struct BsvHomeCore.BsvHomeConfirmSimulation
import enum BsvHomeCore.BsvHomeExecuteResult
import struct BsvHomeCore.BsvHomeSimulationBalanceChange
import struct BsvHomeCore.BsvHomeSimulationValue
import struct BsvHomeCore.BsvHomeTransferData
import struct BsvHomeCore.SimulationPayloadField
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesComponents
import PrimitivesTestKit
import Store
import StoreTestKit
import Testing
@testable import Transfer
import TransferTestKit

@MainActor
struct ConfirmSubmissionTests {
    @Test(.timeLimit(.minutes(1)), arguments: [false, true])
    func cancelledExecutionDiscardsLateResults(broadcastFailure: Bool) async throws {
        let reported = ReportedValues()
        let completed = ReportedValues()
        let request = ConfirmTransferRequest.mock(
            data: .mock(type: .transfer(asset: Asset.mock(id: .mock(.bitcoinSv)).map())),
            delegate: { reported.append(try? $0.get()) },
        )
        let session = BsvHomeConfirmSessionMock()
        let gate = ExecutionGate()
        session.onExecuteAsync = { try await gate.result() }
        let model = ConfirmTransferSceneViewModel.mock(request: request, data: request.data, session: session, onComplete: { completed.append("complete") })
        await model.load()
        model.onSelectConfirm()
        let execution = try #require(model.executionTask)
        await gate.waitUntilStarted()

        model.cancelExecution()
        if broadcastFailure {
            gate.finish(.failure(BsvHomeConfirmError.Broadcast(hashes: ["late-hash"], msg: "late failure")))
        } else {
            gate.finish(.success(.sent(hashes: ["late-hash"], transactions: [Primitives.Transaction.mock().map()])))
        }
        await execution.value

        #expect(reported.values.isEmpty)
        #expect(completed.values.isEmpty)
        #expect(model.bsvReceipt == nil)
        #expect(model.isPresentingAlertMessage == nil)
        #expect(!model.isConfirming)
        #expect(model.executionTask == nil)
        #expect(session.cancelCount == 1)
    }

    @Test(.timeLimit(.minutes(1)))
    func confirmingAfterCancellationStartsANewExecution() async throws {
        let reported = ReportedValues()
        let completed = ReportedValues()
        let request = ConfirmTransferRequest.mock(delegate: { reported.append(try? $0.get()) })
        let session = BsvHomeConfirmSessionMock()
        let first = ExecutionGate()
        let second = ExecutionGate()
        var executions = 0
        session.onExecuteAsync = {
            executions += 1
            return try await (executions == 1 ? first : second).result()
        }
        let model = ConfirmTransferSceneViewModel.mock(request: request, session: session, onComplete: { completed.append("complete") })
        await model.load()
        model.onSelectConfirm()
        let cancelledExecution = try #require(model.executionTask)
        await first.waitUntilStarted()
        model.cancelExecution()
        model.onSelectConfirm()
        let currentExecution = try #require(model.executionTask)
        await second.waitUntilStarted()

        first.finish(.success(.signed(data: ["stale"])))
        await cancelledExecution.value
        #expect(model.executionTask != nil)
        #expect(model.isConfirming)
        #expect(reported.values.isEmpty)
        second.finish(.success(.signed(data: ["fresh"])))
        await currentExecution.value

        #expect(executions == 2)
        #expect(reported.values == ["fresh"])
        #expect(completed.values == ["complete"])
        #expect(model.executionTask == nil)
    }

    @Test
    func bsvReceiptUsesTheSentTransactionAndWaitsForAcknowledgement() async throws {
        let asset = Asset.mock(id: .mock(.bitcoinSv), name: "Bitcoin SV", symbol: "BSV")
        let data = BsvHomeTransferData.mock(
            type: .transfer(asset: asset.map()),
            recipient: .mock(address: "draft-recipient"),
            value: 100_000_000,
        )
        let transaction = Primitives.Transaction.mock(
            id: TransactionId(chain: .bitcoinSv, hash: "actual-transaction-id"),
            assetId: asset.id,
            to: "1BitcoinEaterAddressDontSendf59kuE",
            value: "23456789",
        )
        let reported = ReportedValues()
        let completed = ReportedValues()
        let request = ConfirmTransferRequest.mock(data: data, delegate: { reported.append(try? $0.get()) })
        let model = ConfirmTransferSceneViewModel.mock(
            request: request,
            data: data,
            execute: .success(.sent(hashes: ["broadcast-hash"], transactions: [transaction.map()])),
            onComplete: { completed.append("complete") },
        )
        await model.load()
        model.onSelectConfirm()
        await model.executionTask?.value

        let receipt = try #require(model.bsvReceipt)
        #expect(receipt.recipient == transaction.to)
        #expect(receipt.amountText == "0\(Locale.current.decimalSeparator ?? ".")23456789 BSV")
        #expect(receipt.transactionId == "actual-transaction-id")
        #expect(reported.values == ["broadcast-hash"])
        #expect(completed.values.isEmpty)
        model.completeBsvReceipt()
        #expect(completed.values == ["complete"])
    }

    @Test
    func signedBsvAndSentNonBsvResultsHaveNoBsvReceipt() async {
        let bsv = Asset.mock(id: .mock(.bitcoinSv), name: "Bitcoin SV", symbol: "BSV")
        let signed = ConfirmTransferSceneViewModel.mock(data: .mock(type: .transfer(asset: bsv.map())), execute: .success(.signed(data: ["signed"])))
        let nonBsv = ConfirmTransferSceneViewModel.mock(
            data: .mock(type: .transfer(asset: Asset.mockEthereum().map())),
            execute: .success(.sent(hashes: ["ethereum-hash"], transactions: [Primitives.Transaction.mock().map()])),
        )
        for model in [signed, nonBsv] {
            await model.load()
            model.onSelectConfirm()
            await model.executionTask?.value
            #expect(model.bsvReceipt == nil)
        }
    }

    @Test
    func confirmReportsEveryHashAndTracksSentTransactions() async throws {
        let tracked = Primitives.Transaction.mock()
        let reported = ReportedValues()

        let request = ConfirmTransferRequest.mock(delegate: { reported.append(try? $0.get()) })
        try await ConfirmTransferSceneViewModel.mock(
            request: request,
            execute: .success(.sent(hashes: ["hash-1", "hash-2"], transactions: [tracked.map()])),
        ).submit(request: request)

        #expect(reported.values == ["hash-1", "hash-2"])
    }

    @Test
    func confirmReportsSignedDataWithoutTracking() async throws {
        let reported = ReportedValues()

        let request = ConfirmTransferRequest.mock(delegate: { reported.append(try? $0.get()) })
        try await ConfirmTransferSceneViewModel.mock(request: request, execute: .success(.signed(data: ["signed"]))).submit(request: request)

        #expect(reported.values == ["signed"])
    }

    @Test
    func partialBroadcastReportsBroadcastHashesAndRethrows() async throws {
        let reported = ReportedValues()

        await #expect(throws: BsvHomeConfirmError.self) {
            let request = ConfirmTransferRequest.mock(delegate: { reported.append(try? $0.get()) })
            try await ConfirmTransferSceneViewModel.mock(
                request: request,
                execute: .failure(BsvHomeConfirmError.Broadcast(hashes: ["hash-1"], msg: "second leg failed")),
            ).submit(request: request)
        }

        #expect(reported.values == ["hash-1"])
    }

    @Test
    func simulationStateMapsTheHeader() async {
        let usdt = Asset.mockEthereumUSDT()
        let model = ConfirmTransferSceneViewModel.mock(load: .success(.mock(
            simulation: BsvHomeConfirmSimulation(
                primaryFields: [],
                secondaryFields: [],
                header: BsvHomeSimulationValue(asset: usdt.map(), value: .exact(value: 1_000_000)),
                balanceChanges: [],
                hasCriticalWarning: false,
            ),
        )))
        await model.load()

        let state = model.state.simulation

        #expect(state.headerData == AssetValueHeaderData(asset: usdt, value: .exact(1_000_000)))
        #expect(state.payload.primaryFields.isEmpty)
        #expect(state.payload.secondaryFields.isEmpty)
    }

    @Test
    func simulationStateMapsAnUnlimitedHeader() async {
        let usdt = Asset.mockEthereumUSDT()
        let model = ConfirmTransferSceneViewModel.mock(load: .success(.mock(
            simulation: BsvHomeConfirmSimulation(
                primaryFields: [],
                secondaryFields: [],
                header: BsvHomeSimulationValue(asset: usdt.map(), value: .unlimited),
                balanceChanges: [],
                hasCriticalWarning: false,
            ),
        )))
        await model.load()

        #expect(model.state.simulation.headerData == AssetValueHeaderData(asset: usdt, value: .unlimited))
    }

    @Test
    func simulationStateKeepsPrimaryAndSecondaryFieldsApart() async {
        let primary = SimulationPayloadField.standard(kind: .contract, value: "0x1", fieldType: .text, display: .primary)
        let model = ConfirmTransferSceneViewModel.mock(load: .success(.mock(
            simulation: BsvHomeConfirmSimulation(primaryFields: [primary], secondaryFields: [], header: nil, balanceChanges: [], hasCriticalWarning: false),
        )))
        await model.load()

        let state = model.state.simulation

        #expect(state.payload.primaryFields.count == 1)
        #expect(state.payload.primaryFields.first?.kind == .contract)
        #expect(state.payload.secondaryFields.isEmpty)
    }

    @Test
    func simulationStateMapsBalanceChanges() async {
        let usdt = Asset.mockEthereumUSDT()
        let model = ConfirmTransferSceneViewModel.mock(load: .success(.mock(
            simulation: BsvHomeConfirmSimulation(
                primaryFields: [],
                secondaryFields: [],
                header: nil,
                balanceChanges: [BsvHomeSimulationBalanceChange(asset: usdt.map(), value: "-25", sign: .outgoing)],
                hasCriticalWarning: false,
            ),
        )))
        await model.load()

        #expect(model.state.simulation.balanceChanges == [BsvHomeSimulationBalanceChange(asset: usdt.map(), value: "-25", sign: .outgoing)])
    }

    @MainActor
    private final class ExecutionGate {
        private let started = AsyncStream<Void>.makeStream()
        private var continuation: CheckedContinuation<BsvHomeExecuteResult, any Error>?

        func result() async throws -> BsvHomeExecuteResult {
            try await withCheckedThrowingContinuation { continuation in
                self.continuation = continuation
                started.continuation.yield()
            }
        }

        func waitUntilStarted() async {
            var iterator = started.stream.makeAsyncIterator()
            _ = await iterator.next()
        }

        func finish(_ result: Result<BsvHomeExecuteResult, any Error>) {
            let continuation = continuation
            self.continuation = nil
            continuation?.resume(with: result)
        }
    }
}

private final class ReportedValues: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [String] = []

    var values: [String] { lock.withLock { storage } }

    func append(_ value: String?) {
        guard let value else { return }
        lock.withLock { storage.append(value) }
    }
}
