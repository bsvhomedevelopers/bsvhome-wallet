import struct BsvHomeCore.BsvHomePortalSwapActionNavigation
import enum BsvHomeCore.BsvHomePortalSwapDirection
import enum BsvHomeCore.BsvHomePortalSwapError
import struct BsvHomeCore.BsvHomePortalSwapOrder
import struct BsvHomeCore.BsvHomePortalSwapOrderPage
import struct BsvHomeCore.BsvHomePortalSwapPair
import struct BsvHomeCore.BsvHomePortalSwapPaymentPreparation
import struct BsvHomeCore.BsvHomePortalSwapPaymentRecovery
import struct BsvHomeCore.BsvHomePortalSwapQuote
import protocol BsvHomeCore.BsvHomePortalSwapServiceProtocol
import struct BsvHomeCore.BsvHomeTransactionCheckpointData

public actor BsvHomePortalSwapServiceMock: BsvHomePortalSwapServiceProtocol {
    private let onPairs: @Sendable () async throws -> [BsvHomePortalSwapPair]
    private let onQuote: @Sendable (BsvHomePortalSwapDirection, String) async throws -> BsvHomePortalSwapQuote
    private let onCreateOrder: @Sendable (String) async throws -> BsvHomePortalSwapOrder
    private let onOrders: @Sendable (Bool, UInt32, UInt32) async throws -> BsvHomePortalSwapOrderPage
    private let onRefreshOrder: @Sendable (String) async throws -> BsvHomePortalSwapOrder
    private let onResolveAction: @Sendable (String, String) async throws -> BsvHomePortalSwapActionNavigation
    private let onHasPendingOrder: @Sendable () async throws -> Bool
    private let onPreparePayment: @Sendable (String) async throws -> BsvHomePortalSwapPaymentPreparation
    private let onSavedPayment: @Sendable (String) async throws -> BsvHomeTransactionCheckpointData?
    private let onRecoverPayment: @Sendable (String) async throws -> BsvHomePortalSwapPaymentRecovery
    private let onCancel: @Sendable () -> Void
    private let cached: [BsvHomePortalSwapOrder]
    private var pairsResult: Result<[BsvHomePortalSwapPair], BsvHomePortalSwapError>?
    private var ordersResult: Result<BsvHomePortalSwapOrderPage, BsvHomePortalSwapError>?
    public private(set) var pairsCount = 0
    public private(set) var quoteInputs: [(BsvHomePortalSwapDirection, String)] = []
    public private(set) var createOrderCount = 0
    public private(set) var refreshOrderCount = 0

    public init(
        onPairs: @escaping @Sendable () async throws -> [BsvHomePortalSwapPair] = { [.mock(), .mock(direction: .usdtToBsv), .mock(direction: .usdtToTrx), .mock(direction: .trxToUsdt)] },
        onQuote: @escaping @Sendable (BsvHomePortalSwapDirection, String) async throws -> BsvHomePortalSwapQuote = { direction, _ in .mock(direction: direction) },
        onCreateOrder: @escaping @Sendable (String) async throws -> BsvHomePortalSwapOrder = { _ in .mock() },
        onOrders: @escaping @Sendable (Bool, UInt32, UInt32) async throws -> BsvHomePortalSwapOrderPage = { _, _, _ in BsvHomePortalSwapOrderPage(total: 0, records: []) },
        onRefreshOrder: @escaping @Sendable (String) async throws -> BsvHomePortalSwapOrder = { id in .mock(orderId: id) },
        onResolveAction: @escaping @Sendable (String, String) async throws -> BsvHomePortalSwapActionNavigation = { _, _ in throw BsvHomePortalSwapError.ActionUnavailable },
        onHasPendingOrder: @escaping @Sendable () async throws -> Bool = { false },
        onPreparePayment: @escaping @Sendable (String) async throws -> BsvHomePortalSwapPaymentPreparation = { _ in throw BsvHomePortalSwapError.PaymentUnavailable },
        onSavedPayment: @escaping @Sendable (String) async throws -> BsvHomeTransactionCheckpointData? = { _ in nil },
        onRecoverPayment: @escaping @Sendable (String) async throws -> BsvHomePortalSwapPaymentRecovery = { _ in throw BsvHomePortalSwapError.PaymentUnavailable },
        cachedOrders: [BsvHomePortalSwapOrder] = [],
        onCancel: @escaping @Sendable () -> Void = {},
    ) {
        self.onPairs = onPairs
        self.onQuote = onQuote
        self.onCreateOrder = onCreateOrder
        self.onOrders = onOrders
        self.onRefreshOrder = onRefreshOrder
        self.onResolveAction = onResolveAction
        self.onHasPendingOrder = onHasPendingOrder
        self.onPreparePayment = onPreparePayment
        self.onSavedPayment = onSavedPayment
        self.onRecoverPayment = onRecoverPayment
        cached = cachedOrders
        self.onCancel = onCancel
    }

    public nonisolated func directionForAsset(assetId: String) -> BsvHomePortalSwapDirection? {
        if assetId == BsvHomePortalSwapPair.mock().from.assetId {
            return .bsvToUsdt
        }
        if assetId == BsvHomePortalSwapPair.mock().to.assetId {
            return .usdtToBsv
        }
        return nil
    }

    public func pairs() async throws -> [BsvHomePortalSwapPair] {
        pairsCount += 1
        if let pairsResult {
            return try pairsResult.get()
        }
        return try await onPairs()
    }

    public func setPairsResult(_ result: Result<[BsvHomePortalSwapPair], BsvHomePortalSwapError>) {
        pairsResult = result
    }

    public func quote(direction: BsvHomePortalSwapDirection, decimalAmount: String) async throws -> BsvHomePortalSwapQuote {
        quoteInputs.append((direction, decimalAmount))
        return try await onQuote(direction, decimalAmount)
    }

    public func createOrder(quoteId: String) async throws -> BsvHomePortalSwapOrder {
        createOrderCount += 1
        return try await onCreateOrder(quoteId)
    }

    public func hasPendingOrder() async throws -> Bool {
        try await onHasPendingOrder()
    }

    public func retryPendingOrder() async throws -> BsvHomePortalSwapOrder {
        try await onCreateOrder("pending")
    }

    public func cachedOrders() async throws -> [BsvHomePortalSwapOrder] {
        cached
    }

    public func orders(active: Bool, page: UInt32, size: UInt32) async throws -> BsvHomePortalSwapOrderPage {
        if let ordersResult {
            return try ordersResult.get()
        }
        return try await onOrders(active, page, size)
    }

    public func setOrdersResult(_ result: Result<BsvHomePortalSwapOrderPage, BsvHomePortalSwapError>) {
        ordersResult = result
    }

    public func order(orderId: String) async throws -> BsvHomePortalSwapOrder {
        try await onRefreshOrder(orderId)
    }

    public func refreshOrder(orderId: String) async throws -> BsvHomePortalSwapOrder {
        refreshOrderCount += 1
        return try await onRefreshOrder(orderId)
    }

    public func resolveAction(orderId: String, actionId: String) async throws -> BsvHomePortalSwapActionNavigation {
        try await onResolveAction(orderId, actionId)
    }

    public func preparePayment(orderId: String) async throws -> BsvHomePortalSwapPaymentPreparation {
        try await onPreparePayment(orderId)
    }

    public func savedPayment(orderId: String) async throws -> BsvHomeTransactionCheckpointData? {
        try await onSavedPayment(orderId)
    }

    public func reportSavedBroadcast(orderId: String) async throws -> BsvHomePortalSwapOrder {
        try await onRefreshOrder(orderId)
    }

    public func recoverPayment(orderId: String) async throws -> BsvHomePortalSwapPaymentRecovery {
        try await onRecoverPayment(orderId)
    }

    public func verifyPayout(orderId: String) async throws -> BsvHomePortalSwapOrder {
        try await onRefreshOrder(orderId)
    }

    public nonisolated func cancel() {
        onCancel()
    }
}
