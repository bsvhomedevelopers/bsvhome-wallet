import Foundation
import typealias BsvHomeCore.BsvHomeBigInt
import enum BsvHomeCore.BsvHomePortalSwapError
import struct BsvHomeCore.BsvHomePortalSwapOrder
import struct BsvHomeCore.BsvHomePortalSwapPaymentOrder
import struct BsvHomeCore.BsvHomePortalSwapPaymentPreparation
import struct BsvHomeCore.BsvHomePortalSwapPaymentRecovery
import protocol BsvHomeCore.BsvHomeTransactionCheckpoint
import struct BsvHomeCore.BsvHomeTransactionCheckpointData
import struct BsvHomeCore.BsvHomeTransferData
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Observation
import Primitives
import PrimitivesTestKit
import Testing
@testable import WalletTab

@Suite(.timeLimit(.minutes(1)))
@MainActor
struct PortalSwapPaymentPresenterTests {
    @Test
    func deactivatedPresenterRejectsLateCallback() async {
        let requests = AsyncStream.makeStream(of: String.self)
        let service = BsvHomePortalSwapServiceMock(onSavedPayment: { id in
            requests.continuation.yield(id)
            return nil
        })
        let presenter = PortalSwapPaymentPresenter()
        presenter.deactivate()
        presenter.cancel()

        await #expect(throws: BsvHomePortalSwapError.PaymentUnavailable) {
            try await presenter.present(wallet: .mock(), source: .acquisition(UUID()), orderId: "order", service: service, isActive: { true })
        }

        requests.continuation.finish()
        var received: [String] = []
        for await request in requests.stream {
            received.append(request)
        }
        #expect(received.isEmpty)
        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
    }

    @Test
    func activationAllowsAnotherPresentation() async throws {
        let presenter = PortalSwapPaymentPresenter()
        presenter.deactivate()
        presenter.activate()
        let preparation = preparation()
        let service = BsvHomePortalSwapServiceMock(onPreparePayment: { _ in preparation })
        let task = Task {
            try await presenter.present(wallet: .mock(), source: .acquisition(UUID()), orderId: "order", service: service, isActive: { true })
        }
        let context = try await payment(in: presenter)

        context.finish(.success(()))
        try await task.value
        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
    }

    @Test
    func inactiveSourceDoesNotPreparePayment() async {
        let requests = AsyncStream.makeStream(of: String.self)
        let service = BsvHomePortalSwapServiceMock(onSavedPayment: { id in
            requests.continuation.yield(id)
            return nil
        })
        let presenter = PortalSwapPaymentPresenter()

        await #expect(throws: BsvHomePortalSwapError.PaymentUnavailable) {
            try await presenter.present(wallet: .mock(), source: .tab, orderId: "order", service: service, isActive: { false })
        }

        requests.continuation.finish()
        var received: [String] = []
        for await request in requests.stream {
            received.append(request)
        }
        #expect(received.isEmpty)
        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
    }

    @Test
    func cancellationDuringPreparationPreventsPresentation() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let cancelled = AsyncStream.makeStream(of: Void.self)
        let preparation = preparation()
        let service = BsvHomePortalSwapServiceMock(onPreparePayment: { _ in
            started.continuation.yield(())
            for await _ in response.stream {
                break
            }
            return preparation
        }, onCancel: { cancelled.continuation.yield(()) })
        let presenter = PortalSwapPaymentPresenter()
        let source = PortalSwapPaymentSource.acquisition(UUID())
        let task = Task {
            try await presenter.present(wallet: .mock(), source: source, orderId: "order", service: service, isActive: { true })
        }
        for await _ in started.stream {
            break
        }

        #expect(presenter.source == source)
        presenter.cancel()
        for await _ in cancelled.stream {
            break
        }
        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
        response.continuation.finish()
        await #expect(throws: CancellationError.self) { try await task.value }
        #expect(presenter.payment == nil)
    }

    @Test
    func concurrentPaymentIsRejected() async throws {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let preparation = preparation()
        let service = BsvHomePortalSwapServiceMock(onPreparePayment: { _ in
            started.continuation.yield(())
            for await _ in response.stream {
                break
            }
            return preparation
        })
        let presenter = PortalSwapPaymentPresenter()
        let task = Task {
            try await presenter.present(wallet: .mock(), source: .tab, orderId: "order", service: service, isActive: { true })
        }
        for await _ in started.stream {
            break
        }

        await #expect(throws: BsvHomePortalSwapError.PaymentUnavailable) {
            try await presenter.present(wallet: .mock(), source: .detail(UUID()), orderId: "second", service: service, isActive: { true })
        }

        response.continuation.finish()
        let context = try await payment(in: presenter)
        #expect(context.id == "order")
        context.finish(.success(()))
        try await task.value
        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
    }

    @Test
    func sourceChangeDuringPreparationPreventsPresentation() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let preparation = preparation()
        let service = BsvHomePortalSwapServiceMock(onPreparePayment: { _ in
            started.continuation.yield(())
            for await _ in response.stream {
                break
            }
            return preparation
        })
        let presenter = PortalSwapPaymentPresenter()
        var isActive = true
        let task = Task {
            try await presenter.present(wallet: .mock(), source: .detail(UUID()), orderId: "order", service: service, isActive: { isActive })
        }
        for await _ in started.stream {
            break
        }

        isActive = false
        response.continuation.finish()
        await #expect(throws: CancellationError.self) { try await task.value }
        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
    }

    @Test
    func oldPreparationDoesNotClearNewPayment() async throws {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let oldPreparation = preparation(orderId: "old")
        let oldService = BsvHomePortalSwapServiceMock(onPreparePayment: { _ in
            started.continuation.yield(())
            for await _ in response.stream {
                break
            }
            return oldPreparation
        })
        let presenter = PortalSwapPaymentPresenter()
        let oldTask = Task {
            try await presenter.present(wallet: .mock(), source: .tab, orderId: "old", service: oldService, isActive: { true })
        }
        for await _ in started.stream {
            break
        }
        presenter.cancel()

        let newPreparation = preparation(orderId: "new")
        let newService = BsvHomePortalSwapServiceMock(onPreparePayment: { _ in newPreparation })
        let newSource = PortalSwapPaymentSource.acquisition(UUID())
        let newTask = Task {
            try await presenter.present(wallet: .mock(), source: newSource, orderId: "new", service: newService, isActive: { true })
        }
        let newContext = try await payment(in: presenter)
        response.continuation.finish()
        await #expect(throws: CancellationError.self) { try await oldTask.value }

        #expect(presenter.payment === newContext)
        #expect(presenter.source == newSource)
        newContext.finish(.success(()))
        try await newTask.value
        #expect(presenter.payment == nil)
    }

    @Test
    func taskCancellationClosesPresentedPayment() async throws {
        let cancelled = AsyncStream.makeStream(of: Void.self)
        let preparation = preparation()
        let service = BsvHomePortalSwapServiceMock(onPreparePayment: { _ in preparation }, onCancel: { cancelled.continuation.yield(()) })
        let presenter = PortalSwapPaymentPresenter()
        let task = Task {
            try await presenter.present(wallet: .mock(), source: .tab, orderId: "order", service: service, isActive: { true })
        }
        let context = try await payment(in: presenter)

        task.cancel()
        for await _ in cancelled.stream {
            break
        }
        await #expect(throws: CancellationError.self) { try await task.value }
        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
        context.finish(.success(()))
        #expect(presenter.payment == nil)
    }

    @Test
    func taskCancellationDuringPreparationCleansState() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let cancelled = AsyncStream.makeStream(of: Void.self)
        let preparation = preparation()
        let service = BsvHomePortalSwapServiceMock(onPreparePayment: { _ in
            started.continuation.yield(())
            for await _ in response.stream {
                break
            }
            return preparation
        }, onCancel: { cancelled.continuation.yield(()) })
        let presenter = PortalSwapPaymentPresenter()
        let task = Task {
            try await presenter.present(wallet: .mock(), source: .tab, orderId: "order", service: service, isActive: { true })
        }
        for await _ in started.stream {
            break
        }

        task.cancel()
        for await _ in cancelled.stream {
            break
        }
        #expect(presenter.source == nil)
        response.continuation.finish()
        await #expect(throws: CancellationError.self) { try await task.value }
        #expect(presenter.payment == nil)
    }

    @Test
    func alreadyBroadcastPaymentDoesNotPrepareAgain() async throws {
        var order = BsvHomePortalSwapOrder.mock(direction: .usdtToTrx)
        let saved = savedPayment()
        order.sourceTxid = saved.transactionId
        let recovery = BsvHomePortalSwapPaymentRecovery(order: order, canRebroadcast: false)
        let service = BsvHomePortalSwapServiceMock(onSavedPayment: { _ in saved }, onRecoverPayment: { _ in recovery })
        let presenter = PortalSwapPaymentPresenter()

        try await presenter.present(wallet: .mock(), source: .tab, orderId: "order", service: service, isActive: { true })

        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
    }

    @Test
    func recoveryThatCannotRebroadcastIsRejected() async {
        let saved = savedPayment()
        let recovery = BsvHomePortalSwapPaymentRecovery(order: .mock(direction: .usdtToTrx), canRebroadcast: false)
        let service = BsvHomePortalSwapServiceMock(onSavedPayment: { _ in saved }, onRecoverPayment: { _ in recovery })
        let presenter = PortalSwapPaymentPresenter()

        await #expect(throws: BsvHomePortalSwapError.PaymentUnavailable) {
            try await presenter.present(wallet: .mock(), source: .tab, orderId: "order", service: service, isActive: { true })
        }

        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
    }

    @Test
    func cancellationDuringRecoveryPreventsPreparation() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let saved = savedPayment()
        let recovery = BsvHomePortalSwapPaymentRecovery(order: .mock(direction: .usdtToTrx), canRebroadcast: true)
        let service = BsvHomePortalSwapServiceMock(onSavedPayment: { _ in saved }, onRecoverPayment: { _ in
            started.continuation.yield(())
            for await _ in response.stream {
                break
            }
            return recovery
        })
        let presenter = PortalSwapPaymentPresenter()
        let task = Task {
            try await presenter.present(wallet: .mock(), source: .tab, orderId: "order", service: service, isActive: { true })
        }
        for await _ in started.stream {
            break
        }

        presenter.cancel()
        response.continuation.finish()
        await #expect(throws: CancellationError.self) { try await task.value }
        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
    }

    @Test
    func savedPaymentUsesTheOriginalConfirmationContext() async throws {
        let saved = savedPayment()
        let recovery = BsvHomePortalSwapPaymentRecovery(order: .mock(direction: .usdtToTrx), canRebroadcast: true)
        let preparation = preparation()
        let service = BsvHomePortalSwapServiceMock(onPreparePayment: { _ in preparation }, onSavedPayment: { _ in saved }, onRecoverPayment: { _ in recovery })
        let presenter = PortalSwapPaymentPresenter()
        let task = Task {
            try await presenter.present(wallet: .mock(), source: .tab, orderId: "order", service: service, isActive: { true })
        }
        let context = try await payment(in: presenter)

        #expect(context.savedPayment == saved)
        #expect(context.transfer == preparation.transfer)
        context.finish(.success(()))
        try await task.value
        #expect(presenter.payment == nil)
    }

    @Test
    func paymentForDifferentWalletIsRejected() async {
        let preparation = preparation(wallet: .mock(id: .multicoin(address: "other-wallet")))
        let service = BsvHomePortalSwapServiceMock(onPreparePayment: { _ in preparation })
        let presenter = PortalSwapPaymentPresenter()

        await #expect(throws: BsvHomePortalSwapError.OrderMismatch) {
            try await presenter.present(wallet: .mock(), source: .tab, orderId: "order", service: service, isActive: { true })
        }

        #expect(presenter.payment == nil)
        #expect(presenter.source == nil)
    }

    @Test(arguments: [true, false])
    func completionRunsOnlyOnce(success: Bool) async throws {
        let cancelled = AsyncStream.makeStream(of: Void.self)
        let service = BsvHomePortalSwapServiceMock(onCancel: { cancelled.continuation.yield(()) })
        var completions = 0
        var completedSuccessfully = false
        let context = try PortalSwapPaymentContext(wallet: .mock(), preparation: preparation(), savedPayment: nil, service: service) { result in
            completions += 1
            if case .success = result {
                completedSuccessfully = true
            }
        }

        context.finish(success ? .success(()) : .failure(CancellationError()))
        context.finish(success ? .failure(CancellationError()) : .success(()))
        cancelled.continuation.finish()
        var cancellations = 0
        for await _ in cancelled.stream {
            cancellations += 1
        }

        #expect(completions == 1)
        #expect(completedSuccessfully == success)
        #expect(cancellations == (success ? 0 : 1))
    }

    private func payment(in presenter: PortalSwapPaymentPresenter) async throws -> PortalSwapPaymentContext {
        while presenter.payment == nil {
            let changes = AsyncStream.makeStream(of: Void.self)
            withObservationTracking {
                _ = presenter.payment
            } onChange: {
                changes.continuation.yield(())
                changes.continuation.finish()
            }
            for await _ in changes.stream {
                break
            }
        }
        return try #require(presenter.payment)
    }

    private func preparation(orderId: String = "order", wallet: Wallet = .mock()) -> BsvHomePortalSwapPaymentPreparation {
        let order = BsvHomePortalSwapOrder.mock(orderId: orderId, direction: .usdtToTrx)
        let asset = Asset.mockTronUSDT()
        let depositAddress = "deposit"
        let payment = BsvHomePortalSwapPaymentOrder(
            orderId: order.orderId,
            walletId: wallet.id.identifier,
            fromAssetId: asset.id.identifier,
            fromAmount: order.fromAmount,
            depositAddress: depositAddress,
            depositExtraId: order.depositExtraId,
            payoutAssetId: AssetId(chain: .tron).identifier,
            payoutAddress: order.payoutAddress,
            refundAddress: order.refundAddress,
            estimatedToAmount: order.estimatedToAmount,
            providerDisplayName: order.providerDisplayName,
            depositExpiresAt: order.depositExpiresAt,
            version: order.version,
        )
        return BsvHomePortalSwapPaymentPreparation(
            payment: payment,
            order: order,
            transfer: .mock(type: .transfer(asset), recipient: .mock(address: depositAddress), value: BsvHomeBigInt(order.fromAmount)),
            checkpoint: PaymentCheckpoint(),
        )
    }

    private func savedPayment() -> BsvHomeTransactionCheckpointData {
        BsvHomeTransactionCheckpointData(
            version: 1,
            bsvSpend: nil,
            originalValue: nil,
            useMaxAmount: false,
            gasPrice: nil,
            walletId: Wallet.mock().id.identifier,
            assetId: Asset.mockTronUSDT().id.identifier,
            senderAddress: "sender",
            recipientAddress: "deposit",
            value: 100_000_000,
            networkFee: 1,
            feeAssetId: AssetId(chain: .tron).identifier,
            transactionId: "saved-transaction",
            signedData: "synthetic-test-data",
        )
    }

    private final class PaymentCheckpoint: BsvHomeTransactionCheckpoint {
        func load() async throws -> BsvHomeTransactionCheckpointData? {
            nil
        }

        func save(data _: BsvHomeTransactionCheckpointData) async throws {}
        func authorizeBroadcast(data _: BsvHomeTransactionCheckpointData) async throws {}
    }
}
