import Formatters
import Foundation
import enum BsvHomeCore.BsvHomePortalAuthError
import enum BsvHomeCore.BsvHomePortalSwapDirection
import enum BsvHomeCore.BsvHomePortalSwapError
import struct BsvHomeCore.BsvHomePortalSwapOrder
import struct BsvHomeCore.BsvHomePortalSwapPair
import struct BsvHomeCore.BsvHomePortalSwapQuote
import protocol BsvHomeCore.BsvHomePortalSwapServiceProtocol
import BsvHomeCorePrimitives
import Localization
import Observation
import Primitives

@MainActor
@Observable
public final class WowPayPortalSwapViewModel {
    public enum EntryMode: Equatable, Sendable {
        case exchange
        case acquireTrx
    }

    enum ConnectionState: Equatable {
        case idle
        case connecting
        case connected
        case cancelled
        case failed(String)
    }

    enum Operation: Equatable {
        case idle
        case creating
        case paying
        case refreshing
        case openingAction
    }

    private enum EntryAsset {
        case detail(AssetId)
        case home(WowPayAsset)
    }

    private let makeService: @Sendable () throws -> any BsvHomePortalSwapServiceProtocol
    private let onPayment: @MainActor (String, any BsvHomePortalSwapServiceProtocol) async throws -> Void
    private let entryMode: EntryMode
    private var service: (any BsvHomePortalSwapServiceProtocol)?
    private var activationId: UUID?
    private var quoteRequestId: UUID?
    private var ordersRequestId: UUID?
    private var quoteTask: Task<Void, Never>?
    private var quoteExpiryTask: Task<Void, Never>?
    private var pendingEntryAsset: EntryAsset?

    let selection = WowPaySwapSelectionViewModel()
    var amount = ""
    var selectedOrder: BsvHomePortalSwapOrder?
    var actionUrl: URL?
    var showsAllOrders = false
    private(set) var connection: ConnectionState = .idle
    private(set) var operation: Operation = .idle
    private(set) var pairs: [BsvHomePortalSwapPair] = []
    private(set) var quote: BsvHomePortalSwapQuote?
    private(set) var orders: [BsvHomePortalSwapOrder] = []
    private(set) var totalOrders: UInt64 = 0
    private var ordersPage: UInt32 = 1
    private(set) var isLoadingQuote = false
    private(set) var isLoadingOrders = false
    private(set) var hasPendingOrder = false
    private(set) var isEntryAssetUnsupported = false
    private(set) var quoteError: String?
    private(set) var orderError: String?
    private(set) var requestVersion = 0

    public init(
        makeService: @escaping @Sendable () throws -> any BsvHomePortalSwapServiceProtocol,
        onPayment: @escaping @MainActor (String, any BsvHomePortalSwapServiceProtocol) async throws -> Void,
        entryMode: EntryMode = .exchange,
    ) {
        self.makeService = makeService
        self.onPayment = onPayment
        self.entryMode = entryMode
        if entryMode == .acquireTrx {
            selection.selectPair(from: .usdt, to: .trx)
        }
    }

    var isTargetLocked: Bool { entryMode == .acquireTrx }
    var direction: BsvHomePortalSwapDirection? { pair?.direction }
    var isBusy: Bool { operation != .idle }
    var pair: BsvHomePortalSwapPair? { pair(from: selection.from, to: selection.to) }
    var canCreateOrder: Bool {
        connection == .connected && direction != nil && quote?.direction == direction && (quote?.expiresAt ?? 0) > Int64(Date().timeIntervalSince1970) && !isLoadingQuote && !isBusy && !hasPendingOrder
    }

    var canReverse: Bool { connection == .connected && hasPair(from: selection.to, to: selection.from) && !isBusy }
    var canUseMaximum: Bool { connection == .connected && selection.from == .usdt && direction != nil && !isBusy }
    var hasMoreOrders: Bool { UInt64(orders.count) < totalOrders }
    var hasNoOrders: Bool { connection == .connected && orders.isEmpty && !isLoadingOrders && orderError == nil }

    var selectableAssets: [WowPayAsset] { selection.side.map { assets(for: $0) } ?? [] }

    func assets(for side: WowPaySwapSelectionViewModel.Side) -> [WowPayAsset] {
        switch side {
        case .from: WowPayAsset.visibleCases.filter { asset in selectablePairs.contains { $0.from.assetId == asset.assetId?.identifier } }
        case .to: isTargetLocked ? [] : destinations(for: selection.from)
        }
    }

    private var selectablePairs: [BsvHomePortalSwapPair] {
        switch entryMode {
        case .exchange: pairs
        case .acquireTrx: pairs.filter { $0.to.assetId == WowPayAsset.trx.assetId?.identifier }
        }
    }

    private func destinations(for asset: WowPayAsset) -> [WowPayAsset] {
        WowPayAsset.visibleCases.filter { hasPair(from: asset, to: $0) }
    }

    func hasPair(from: WowPayAsset, to: WowPayAsset) -> Bool {
        pair(from: from, to: to) != nil
    }

    private func pair(from: WowPayAsset, to: WowPayAsset) -> BsvHomePortalSwapPair? {
        selectablePairs.first { $0.from.assetId == from.assetId?.identifier && $0.to.assetId == to.assetId?.identifier }
    }

    func load() async {
        guard connection == .idle else { return }
        let activationId = UUID()
        self.activationId = activationId
        connection = .connecting
        pairs = []
        invalidateQuote()
        do {
            let service = try service ?? makeService()
            self.service = service
            let cachedOrders = try await service.cachedOrders()
            try Task.checkCancellation()
            guard self.activationId == activationId else { return }
            orders = cachedOrders
            let hasPendingOrder = try await service.hasPendingOrder()
            guard self.activationId == activationId else { return }
            self.hasPendingOrder = hasPendingOrder
            let pairs = try await service.pairs()
            try Task.checkCancellation()
            guard self.activationId == activationId else { return }
            self.pairs = pairs
            connection = .connected
            applyEntryAsset(service)
            await reloadOrders()
            amountChanged()
        } catch {
            guard self.activationId == activationId else { return }
            connection = isCancelled(error) ? .cancelled : .failed(message(error))
        }
    }

    func retryConnection() {
        guard connection != .connecting else { return }
        connection = .idle
        requestVersion += 1
    }

    func refresh() async {
        if case .failed = connection {
            retryConnection()
            await load()
        } else if connection == .idle {
            await load()
        } else if connection == .connected {
            await reloadPairs()
        }
        quoteTask?.cancel()
        await reloadQuote()
        await refreshActiveOrders()
    }

    func retryQuote() async {
        await reloadPairs()
        quoteTask?.cancel()
        await reloadQuote()
    }

    private func reloadPairs() async {
        guard let service, connection == .connected, let activationId, !isBusy else { return }
        connection = .connecting
        pairs = []
        invalidateQuote()
        do {
            let pairs = try await service.pairs()
            try Task.checkCancellation()
            guard self.activationId == activationId else { return }
            self.pairs = pairs
            connection = .connected
            applyEntryAsset(service)
        } catch {
            guard self.activationId == activationId else { return }
            connection = isCancelled(error) ? .cancelled : .failed(message(error))
        }
    }

    public func onDisappear() {
        service?.cancel()
        activationId = nil
        ordersRequestId = nil
        invalidateQuote()
        pairs = []
        isLoadingOrders = false
        operation = .idle
        switch connection {
        case .idle, .connecting, .connected: connection = .idle
        case .cancelled, .failed: break
        }
    }

    func amountChanged() {
        invalidateQuote()
        guard connection == .connected, direction != nil, !amount.isEmpty, !isBusy else { return }
        quoteTask = Task { [weak self] in
            do {
                try await Task.sleep(for: .milliseconds(600))
                await self?.reloadQuote()
            } catch is CancellationError {}
            catch {
                self?.quoteError = error.localizedDescription
            }
        }
    }

    private func invalidateQuote() {
        quoteTask?.cancel()
        quoteTask = nil
        quoteExpiryTask?.cancel()
        quoteExpiryTask = nil
        quoteRequestId = nil
        quote = nil
        quoteError = nil
        isLoadingQuote = false
    }

    func selectAsset(_ asset: WowPayAsset) {
        guard !isBusy, let side = selection.side, assets(for: side).contains(asset) else { return }
        switch side {
        case .from:
            selectPair(from: asset, to: destination(for: asset))
        case .to:
            selectPair(from: selection.from, to: asset)
        }
    }

    func reverse() {
        guard canReverse else { return }
        selectPair(from: selection.to, to: selection.from)
    }

    func selectPair(from: WowPayAsset, to: WowPayAsset) {
        guard !isBusy, connection == .connected, hasPair(from: from, to: to) else { return }
        selection.selectPair(from: from, to: to)
        amount = ""
        amountChanged()
    }

    func selectEntryAsset(_ assetId: AssetId?) {
        guard entryMode == .exchange, let assetId else { return }
        selectedOrder = nil
        isEntryAssetUnsupported = false
        pendingEntryAsset = .detail(assetId)
        if let service {
            applyEntryAsset(service)
        }
    }

    func selectHomeSourceAsset(_ asset: WowPayAsset) {
        guard entryMode == .exchange else { return }
        selectedOrder = nil
        isEntryAssetUnsupported = false
        pendingEntryAsset = .home(asset)
        selection.selectPair(from: asset, to: selection.to)
        amount = ""
        amountChanged()
        if let service, connection == .connected {
            applyEntryAsset(service)
        }
    }

    private func destination(for asset: WowPayAsset) -> WowPayAsset {
        let destinations = destinations(for: asset)
        return destinations.contains(selection.to) ? selection.to : destinations.first(where: { $0 == .usdt }) ?? destinations.first ?? selection.to
    }

    private func applyEntryAsset(_ service: any BsvHomePortalSwapServiceProtocol) {
        guard let entryAsset = pendingEntryAsset else { return }
        switch entryAsset {
        case let .home(asset):
            guard connection == .connected else { return }
            selection.selectPair(from: asset, to: destination(for: asset))
        case let .detail(assetId):
            let direction = service.directionForAsset(assetId: assetId.identifier)
            isEntryAssetUnsupported = direction == nil
            switch direction {
            case .bsvToUsdt: selection.selectPair(from: .bsv, to: .usdt)
            case .usdtToBsv: selection.selectPair(from: .usdt, to: .bsv)
            case .bsvToTrx: selection.selectPair(from: .bsv, to: .trx)
            case .trxToBsv: selection.selectPair(from: .trx, to: .bsv)
            case .usdtToTrx: selection.selectPair(from: .usdt, to: .trx)
            case .trxToUsdt: selection.selectPair(from: .trx, to: .usdt)
            case nil: break
            }
        }
        pendingEntryAsset = nil
        amount = ""
        amountChanged()
    }

    func useMaximum(_ amount: String) {
        guard canUseMaximum else { return }
        self.amount = amount
        amountChanged()
    }

    func reloadQuote() async {
        guard let service, let direction, connection == .connected, activationId != nil, !amount.isEmpty, !isBusy else { return }
        let requestId = UUID()
        let activationId = self.activationId
        quoteRequestId = requestId
        quote = nil
        quoteExpiryTask?.cancel()
        quoteError = nil
        isLoadingQuote = true
        do {
            let quote = try await service.quote(direction: direction, decimalAmount: ValueFormatter(style: .full).plainInputNumber(amount))
            try Task.checkCancellation()
            guard self.activationId == activationId, quoteRequestId == requestId else { return }
            guard quote.expiresAt > Int64(Date().timeIntervalSince1970) else {
                quoteError = Localized.wowpaySwapQuoteExpired
                isLoadingQuote = false
                return
            }
            self.quote = quote
            isLoadingQuote = false
            expireQuote(quote, requestId: requestId)
        } catch {
            guard self.activationId == activationId, quoteRequestId == requestId else { return }
            quoteError = message(error)
            isLoadingQuote = false
        }
    }

    func createOrder() async {
        guard canCreateOrder, let service, let quote else { return }
        operation = .creating
        orderError = nil
        let activationId = self.activationId
        do {
            let order = try await service.createOrder(quoteId: quote.quoteId)
            guard self.activationId == activationId else { return }
            upsertOrder(order)
            selectedOrder = order
            self.quote = nil
            quoteExpiryTask?.cancel()
            quoteRequestId = nil
            let pending = try await service.hasPendingOrder()
            guard self.activationId == activationId else { return }
            hasPendingOrder = pending
        } catch {
            guard self.activationId == activationId else { return }
            orderError = message(error)
            do {
                let pending = try await service.hasPendingOrder()
                guard self.activationId == activationId else { return }
                hasPendingOrder = pending
            } catch {
                guard self.activationId == activationId else { return }
                hasPendingOrder = true
                orderError = [orderError, message(error)].compactMap(\.self).joined(separator: "\n")
            }
        }
        guard self.activationId == activationId else { return }
        operation = .idle
    }

    func recoverPendingOrder() async {
        guard entryMode == .exchange, let service, connection == .connected, activationId != nil, !isBusy, hasPendingOrder else { return }
        operation = .creating
        orderError = nil
        let activationId = self.activationId
        do {
            let order = try await service.retryPendingOrder()
            guard self.activationId == activationId else { return }
            upsertOrder(order)
            selectedOrder = order
            let pending = try await service.hasPendingOrder()
            guard self.activationId == activationId else { return }
            hasPendingOrder = pending
        } catch {
            guard self.activationId == activationId else { return }
            orderError = message(error)
        }
        guard self.activationId == activationId else { return }
        operation = .idle
    }

    func reloadOrders() async {
        guard let service, connection == .connected, activationId != nil else { return }
        let activationId = self.activationId
        let showsAllOrders = showsAllOrders
        let requestId = UUID()
        ordersRequestId = requestId
        isLoadingOrders = true
        orderError = nil
        do {
            let page = try await service.orders(active: !showsAllOrders, page: 1, size: 20)
            guard self.activationId == activationId, ordersRequestId == requestId else { return }
            orders = page.records
            totalOrders = page.total
            ordersPage = 1
            let pending = try await service.hasPendingOrder()
            guard self.activationId == activationId, ordersRequestId == requestId else { return }
            hasPendingOrder = pending
        } catch {
            guard self.activationId == activationId, ordersRequestId == requestId else { return }
            orderError = ordersMessage(error)
        }
        guard self.activationId == activationId, ordersRequestId == requestId else { return }
        isLoadingOrders = false
    }

    func loadMoreOrders() async {
        guard let service, connection == .connected, activationId != nil, !isLoadingOrders, hasMoreOrders else { return }
        let activationId = self.activationId
        let pageNumber = ordersPage + 1
        let requestId = UUID()
        ordersRequestId = requestId
        isLoadingOrders = true
        orderError = nil
        do {
            let page = try await service.orders(active: !showsAllOrders, page: pageNumber, size: 20)
            guard self.activationId == activationId, ordersRequestId == requestId else { return }
            let existingIds = Set(orders.map(\.orderId))
            orders.append(contentsOf: page.records.filter { !existingIds.contains($0.orderId) })
            totalOrders = page.total
            ordersPage = pageNumber
            let pending = try await service.hasPendingOrder()
            guard self.activationId == activationId, ordersRequestId == requestId else { return }
            hasPendingOrder = pending
        } catch {
            guard self.activationId == activationId, ordersRequestId == requestId else { return }
            orderError = ordersMessage(error)
        }
        guard self.activationId == activationId, ordersRequestId == requestId else { return }
        isLoadingOrders = false
    }

    func refreshOrder(_ orderId: String) async {
        guard let service, connection == .connected, activationId != nil, !isBusy else { return }
        operation = .refreshing
        orderError = nil
        let activationId = self.activationId
        do {
            let order = try await service.refreshOrder(orderId: orderId)
            guard self.activationId == activationId else { return }
            upsertOrder(order)
        } catch {
            guard self.activationId == activationId else { return }
            orderError = message(error)
        }
        guard self.activationId == activationId else { return }
        operation = .idle
    }

    func refreshActiveOrders() async {
        guard connection == .connected, activationId != nil else { return }
        if let order = selectedOrder ?? orders.first(where: { $0.resolutionStatus != .settled }) {
            await refreshOrder(order.orderId)
        }
        await reloadOrders()
    }

    func canPayOrder(_ order: BsvHomePortalSwapOrder) -> Bool {
        connection == .connected && activationId != nil && !isBusy && order.canPay && (!isTargetLocked || order.to?.assetId == WowPayAsset.trx.assetId?.identifier)
    }

    func payOrder(_ order: BsvHomePortalSwapOrder) async {
        guard let service, canPayOrder(order) else { return }
        operation = .paying
        orderError = nil
        var paymentError: String?
        let activationId = self.activationId
        do {
            try await onPayment(order.orderId, service)
        } catch {
            guard self.activationId == activationId else { return }
            if isCancelled(error) {
                operation = .idle
                return
            }
            paymentError = message(error)
        }
        guard self.activationId == activationId else { return }
        operation = .idle
        await refreshOrder(order.orderId)
        guard self.activationId == activationId else { return }
        if let paymentError {
            orderError = [paymentError, orderError].compactMap(\.self).joined(separator: "\n")
        }
    }

    func openAction(orderId: String, actionId: String) async {
        guard let service, connection == .connected, activationId != nil, !isBusy else { return }
        operation = .openingAction
        orderError = nil
        let activationId = self.activationId
        do {
            let action = try await service.resolveAction(orderId: orderId, actionId: actionId)
            guard self.activationId == activationId else { return }
            actionUrl = URL(string: action.navigationUrl)
        } catch {
            guard self.activationId == activationId else { return }
            orderError = message(error)
        }
        guard self.activationId == activationId else { return }
        operation = .idle
    }

    func verifyPayout(_ orderId: String) async {
        guard let service, connection == .connected, activationId != nil, !isBusy else { return }
        operation = .refreshing
        orderError = nil
        let activationId = self.activationId
        do {
            let order = try await service.verifyPayout(orderId: orderId)
            guard self.activationId == activationId else { return }
            upsertOrder(order)
        } catch {
            guard self.activationId == activationId else { return }
            orderError = message(error)
        }
        guard self.activationId == activationId else { return }
        operation = .idle
    }

    private func upsertOrder(_ order: BsvHomePortalSwapOrder) {
        orders.removeAll { $0.orderId == order.orderId }
        orders.insert(order, at: 0)
        if selectedOrder?.orderId == order.orderId {
            selectedOrder = order
        }
    }

    private func expireQuote(_ quote: BsvHomePortalSwapQuote, requestId: UUID) {
        let duration = max(0, Date(timeIntervalSince1970: TimeInterval(quote.expiresAt)).timeIntervalSinceNow)
        quoteExpiryTask = Task { [weak self] in
            do {
                try await Task.sleep(for: .seconds(duration))
            } catch {
                return
            }
            guard let self, quoteRequestId == requestId else { return }
            self.quote = nil
            quoteError = Localized.wowpaySwapQuoteExpired
        }
    }

    private func isCancelled(_ error: Error) -> Bool {
        if case let BsvHomePortalSwapError.Auth(error) = error {
            return isCancelled(error)
        }
        if case let BsvHomePortalSwapError.Service(error) = error {
            return error.isCancelled
        }
        if case let BsvHomePortalAuthError.Service(error) = error {
            return error.isCancelled
        }
        return error.isCancelled
    }

    private func message(_ error: Error) -> String {
        switch error {
        case let BsvHomePortalSwapError.Auth(error): message(error)
        case let BsvHomePortalSwapError.Service(error): error.localizedDescription
        case BsvHomePortalSwapError.MissingAccount: Localized.wowpaySwapAccountRequired
        case BsvHomePortalSwapError.QuoteExpired: Localized.wowpaySwapQuoteExpired
        case BsvHomePortalSwapError.PendingCreation: Localized.wowpaySwapRetryOrder
        case BsvHomePortalSwapError.PaymentUnavailable, BsvHomePortalSwapError.OrderMismatch, BsvHomePortalSwapError.BroadcastConflict: Localized.wowpaySwapOrderContextUnavailable
        case BsvHomePortalSwapError.UnsupportedPair: Localized.wowpayUnavailable
        case BsvHomePortalSwapError.InvalidAmount: Localized.Errors.invalidAmount
        case BsvHomePortalSwapError.InvalidResponse: Localized.wowpaySwapInvalidResponse
        case BsvHomePortalSwapError.ActionUnavailable: Localized.Errors.unknown
        case let BsvHomePortalSwapError.InsufficientBalance(symbol, available, required):
            [Localized.Transfer.insufficientBalance(symbol), Localized.Transfer.balance(available), "\(Localized.Swap.youPay): \(required)"].joined(separator: "\n")
        case BsvHomePortalSwapError.NetworkFeeUnavailable: Localized.Transfer.insufficientNetworkFeeBalance("TRX")
        case BsvHomePortalSwapError.PayoutUnconfirmed: Localized.wowpaySwapPayoutVerificationPending
        case let BsvHomePortalAuthError.Service(error): error.localizedDescription
        case BsvHomePortalAuthError.MissingBsvAccount: Localized.Errors.walletAccountMissing
        case BsvHomePortalAuthError.WatchOnly: Localized.Wallet.Watch.Tooltip.title
        case let BsvHomePortalAuthError.Api(code, message): apiMessage(code: code, message: message)
        case BsvHomePortalAuthError.InvalidResponse: Localized.wowpaySwapInvalidResponse
        case BsvHomePortalAuthError.WalletChanged, BsvHomePortalAuthError.InvalidChallenge, BsvHomePortalAuthError.InvalidSession: Localized.Errors.unknown
        default: error.localizedDescription
        }
    }

    private func ordersMessage(_ error: Error) -> String {
        [Localized.wowpaySwapOrdersLoadFailed, message(error)].joined(separator: "\n")
    }

    private func apiMessage(code: String, message: String) -> String {
        let tryAgain = "\(Localized.Errors.unknown)\n\(Localized.Common.tryAgain)"
        switch code {
        case "SWAP_PROVIDER_UNAVAILABLE", "SWAP_PAIR_UNAVAILABLE": return Localized.Info.NoQuote.description
        case "SWAP_AMOUNT_OUT_OF_RANGE": return Localized.Errors.invalidAmount
        case "SWAP_QUOTE_EXPIRED": return Localized.wowpaySwapQuoteExpired
        case "SYS_REQUEST_LIMITED", "SWAP_RATE_LIMITED", "SYS_SERVER_ERROR": return tryAgain
        default:
            let message = message.trimmingCharacters(in: .whitespacesAndNewlines)
            return message.isEmpty || message.hasPrefix("RSP_MSG.") || message.hasSuffix("#I18N") ? tryAgain : message
        }
    }
}
