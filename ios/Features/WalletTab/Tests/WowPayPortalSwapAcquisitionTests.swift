import struct BsvHomeCore.BsvHomePortalSwapOrder
import struct BsvHomeCore.BsvHomePortalSwapOrderPage
import BsvHomeCoreServicesTestKit
import Testing
@testable import WalletTab

@MainActor
struct WowPayPortalSwapAcquisitionTests {
    @Test
    func defaultsAndChoicesAcquireTrx() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = acquisitionModel(service: service)
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .trx)
        #expect(model.amount.isEmpty)
        await model.load()
        #expect(model.assets(for: .from).isEmpty)
        #expect(model.assets(for: .to).isEmpty)
        #expect(model.hasPair(from: .usdt, to: .trx))
        #expect(!model.hasPair(from: .bsv, to: .usdt))
        #expect(!model.hasPair(from: .trx, to: .usdt))
        #expect(!model.canReverse)
        model.onDisappear()
    }

    @Test
    func exchangeActionsCannotChangeAcquisitionTarget() async {
        let service = BsvHomePortalSwapServiceMock(onCreateOrder: { _ in .mock(direction: .usdtToTrx) })
        let model = acquisitionModel(service: service)
        await model.load()
        model.selectPair(from: .bsv, to: .usdt)
        model.selection.side = .from
        model.selectAsset(.bsv)
        model.selectAsset(.trx)
        model.selection.side = .to
        model.selectAsset(.bsv)
        model.reverse()
        model.selectEntryAsset(WowPayAsset.bsv.assetId)
        model.selectHomeSourceAsset(.bsv)
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .trx)
        model.amount = "2.5"
        await model.reloadQuote()
        await model.createOrder()
        #expect(await service.quoteInputs.first?.0 == .usdtToTrx)
        #expect(await service.createOrderCount == 1)
        #expect(model.selectedOrder?.direction == .usdtToTrx)
        model.onDisappear()
    }

    @Test
    func unavailableTargetCannotQuoteOrCreateOrder() async {
        let service = BsvHomePortalSwapServiceMock(onPairs: { [.mock(), .mock(direction: .usdtToBsv)] })
        let model = acquisitionModel(service: service)
        await model.load()
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .trx)
        #expect(model.assets(for: .from).isEmpty)
        model.selectPair(from: .usdt, to: .bsv)
        model.amount = "2.5"
        await model.reloadQuote()
        await model.createOrder()
        #expect(model.pair == nil)
        #expect(await service.quoteInputs.isEmpty)
        #expect(await service.createOrderCount == 0)
        model.onDisappear()
    }

    @Test
    func quoteAndOrderGuardRejectSelectionOutsideTarget() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = acquisitionModel(service: service)
        await model.load()
        model.amount = "2.5"
        await model.reloadQuote()
        #expect(model.canCreateOrder)
        model.selection.selectPair(from: .bsv, to: .usdt)
        await model.reloadQuote()
        await model.createOrder()
        #expect(await service.quoteInputs.count == 1)
        #expect(await service.createOrderCount == 0)
        #expect(!model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func targetSurvivesCapabilityRemovalRetryAndReturn() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = acquisitionModel(service: service)
        await model.load()
        model.amount = "2.5"
        await model.reloadQuote()
        await service.setPairsResult(.success([.mock()]))
        await model.refresh()
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .trx)
        #expect(model.pair == nil)
        #expect(model.quote == nil)
        #expect(await service.quoteInputs.count == 1)
        await service.setPairsResult(.success([.mock(direction: .usdtToTrx), .mock(direction: .trxToUsdt)]))
        model.retryConnection()
        await model.load()
        #expect(model.assets(for: .from).isEmpty)
        #expect(model.selection.to == .trx)
        #expect(!model.canReverse)
        await model.retryQuote()
        #expect(model.quote?.direction == .usdtToTrx)
        model.onDisappear()
        await model.load()
        #expect(model.selection.to == .trx)
        #expect(model.assets(for: .to).isEmpty)
        model.onDisappear()
    }

    @Test
    func historicalOrdersRemainAvailable() async {
        let order = BsvHomePortalSwapOrder.mock(direction: .bsvToUsdt)
        let service = BsvHomePortalSwapServiceMock(onOrders: { _, _, _ in BsvHomePortalSwapOrderPage(total: 1, records: [order]) }, cachedOrders: [order])
        let model = acquisitionModel(service: service)
        await model.load()
        #expect(model.orders == [order])
        #expect(model.totalOrders == 1)
        #expect(model.assets(for: .from).isEmpty)
        model.onDisappear()
    }

    @Test
    func historicalPaymentMustStillAcquireTrx() async {
        let service = BsvHomePortalSwapServiceMock()
        var paidOrders: [String] = []
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { orderId, _ in
            paidOrders.append(orderId)
        }, entryMode: .acquireTrx)
        await model.load()
        let historicalOrder = BsvHomePortalSwapOrder.mock(direction: .bsvToUsdt)
        #expect(!model.canPayOrder(historicalOrder))
        await model.payOrder(historicalOrder)
        #expect(paidOrders.isEmpty)
        let trxOrder = BsvHomePortalSwapOrder.mock(orderId: "trx-order", direction: .usdtToTrx)
        #expect(model.canPayOrder(trxOrder))
        await model.payOrder(trxOrder)
        #expect(paidOrders == ["trx-order"])
        model.onDisappear()
    }

    @Test
    func opaquePendingOperationCannotCreateAcquisitionOrder() async {
        let service = BsvHomePortalSwapServiceMock(onHasPendingOrder: { true })
        let model = acquisitionModel(service: service)
        await model.load()
        #expect(model.hasPendingOrder)
        model.amount = "2.5"
        await model.reloadQuote()
        #expect(model.quote != nil)
        #expect(!model.canCreateOrder)
        await model.createOrder()
        await model.recoverPendingOrder()
        #expect(await service.createOrderCount == 0)
        #expect(model.hasPendingOrder)
        #expect(model.selectedOrder == nil)
        model.onDisappear()
    }

    private func acquisitionModel(service: BsvHomePortalSwapServiceMock) -> WowPayPortalSwapViewModel {
        WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in }, entryMode: .acquireTrx)
    }
}
