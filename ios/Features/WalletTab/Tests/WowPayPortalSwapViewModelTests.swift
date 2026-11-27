import Foundation
import enum BsvHomeCore.BsvHomePortalAuthError
import struct BsvHomeCore.BsvHomePortalSwapActionNavigation
import enum BsvHomeCore.BsvHomePortalSwapDirection
import enum BsvHomeCore.BsvHomePortalSwapError
import struct BsvHomeCore.BsvHomePortalSwapOrder
import struct BsvHomeCore.BsvHomePortalSwapOrderPage
import struct BsvHomeCore.BsvHomePortalSwapPair
import BsvHomeCoreServicesTestKit
import Localization
import Primitives
import Testing
@testable import WalletTab

@MainActor
struct WowPayPortalSwapViewModelTests {
    @Test(arguments: [
        (WowPayAsset.bsv, WowPayAsset.usdt, BsvHomePortalSwapDirection.bsvToUsdt),
        (.trx, .usdt, .trxToUsdt),
        (.usdt, .bsv, .usdtToBsv),
    ])
    func homeSourceDefaults(from: WowPayAsset, to: WowPayAsset, direction: BsvHomePortalSwapDirection) async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        model.selectHomeSourceAsset(from)
        await model.load()
        #expect(model.selection.from == from)
        #expect(model.selection.to == to)
        model.amount = "2.5"
        await model.reloadQuote()
        #expect(await service.quoteInputs.first?.0 == direction)
        #expect(model.quote?.direction == direction)
        model.onDisappear()
    }

    @Test
    func homeSourceWaitsForPairsAndUsesLatestChoice() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let service = BsvHomePortalSwapServiceMock(onPairs: {
            started.continuation.yield(())
            for await _ in response.stream {
                break
            }
            return [.mock(direction: .usdtToBsv)]
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        let task = Task { await model.load() }
        for await _ in started.stream {
            break
        }
        model.selectHomeSourceAsset(.trx)
        model.selectHomeSourceAsset(.usdt)
        #expect(model.connection == .connecting)
        #expect(model.selection.from == .usdt)
        #expect(model.pair == nil)
        response.continuation.finish()
        await task.value
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .bsv)
        model.amount = "2.5"
        await model.reloadQuote()
        #expect(await service.quoteInputs.first?.0 == .usdtToBsv)
        model.onDisappear()
    }

    @Test
    func missingHomeSourceRemainsUnavailable() async {
        let service = BsvHomePortalSwapServiceMock(onPairs: { [.mock(), .mock(direction: .usdtToBsv)] })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        model.selectHomeSourceAsset(.trx)
        await model.load()
        #expect(model.selection.from == .trx)
        #expect(model.pair == nil)
        model.amount = "2.5"
        await model.reloadQuote()
        await model.createOrder()
        #expect(await service.quoteInputs.isEmpty)
        #expect(await service.createOrderCount == 0)
        #expect(!model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func ordinaryTabReturnRetainsSelectedPair() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.selectPair(from: .usdt, to: .trx)
        model.onDisappear()
        await model.load()
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .trx)
        #expect(model.direction == .usdtToTrx)
        model.onDisappear()
    }

    @Test
    func repeatedHomeSourceClearsPreviousInputAndQuote() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        model.selectHomeSourceAsset(.usdt)
        await model.load()
        model.selectPair(from: .usdt, to: .trx)
        model.amount = "2.5"
        await model.reloadQuote()
        #expect(model.quote != nil)
        model.selectedOrder = .mock()
        model.selectHomeSourceAsset(.usdt)
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .bsv)
        #expect(model.amount.isEmpty)
        #expect(model.quote == nil)
        #expect(model.selectedOrder == nil)
        await model.reloadQuote()
        #expect(await service.quoteInputs.count == 1)
        model.onDisappear()
    }

    @Test
    func homeSourceDiscardsObsoleteQuoteResponse() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let service = BsvHomePortalSwapServiceMock(onQuote: { direction, _ in
            started.continuation.yield(())
            for await _ in response.stream {
                break
            }
            return .mock(direction: direction)
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "2.5"
        let task = Task { await model.reloadQuote() }
        for await _ in started.stream {
            break
        }
        model.selectHomeSourceAsset(.usdt)
        response.continuation.finish()
        await task.value
        #expect(model.selection.from == .usdt)
        #expect(model.amount.isEmpty)
        #expect(model.quote == nil)
        #expect(!model.canCreateOrder)
        model.onDisappear()
    }

    @Test(arguments: [
        (WowPayAsset.bsv, WowPayAsset.usdt, BsvHomePortalSwapDirection.bsvToUsdt),
        (.usdt, .bsv, .usdtToBsv),
        (.usdt, .trx, .usdtToTrx),
        (.trx, .usdt, .trxToUsdt),
    ])
    func supportedQuoteDirection(from: WowPayAsset, to: WowPayAsset, direction: BsvHomePortalSwapDirection) async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.selectPair(from: from, to: to)
        model.amount = "2.5"
        await model.reloadQuote()
        #expect(await service.quoteInputs.first?.0 == direction)
        #expect(await service.quoteInputs.first?.1 == "2.5")
        #expect(model.quote?.direction == direction)
        #expect(model.canCreateOrder)
        #expect(model.canReverse)
        #expect(model.canUseMaximum == (from == .usdt))
        model.onDisappear()
    }

    @Test
    func menusIntersectReturnedPairsWithBsvProductScope() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        #expect(model.assets(for: .from) == [.bsv])
        #expect(model.assets(for: .to).isEmpty)
        #expect(!model.hasPair(from: .bsv, to: .trx))
        #expect(!model.hasPair(from: .trx, to: .bsv))
        model.selectPair(from: .usdt, to: .bsv)
        #expect(model.assets(for: .to) == [.bsv])
        model.selection.side = .from
        model.selectAsset(.mnee)
        model.selectPair(from: .bsv, to: .trx)
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .bsv)
        model.onDisappear()
    }

    @Test
    func hiddenSourcePreservesLegacyQuoteUntilAnExplicitPairChange() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.selectPair(from: .usdt, to: .bsv)
        model.amount = "2.5"
        await model.reloadQuote()
        model.selection.side = .from
        model.selectAsset(.trx)
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .bsv)
        #expect(model.amount == "2.5")
        #expect(model.quote?.direction == .usdtToBsv)
        model.selectPair(from: .bsv, to: .usdt)
        #expect(model.amount.isEmpty)
        #expect(model.quote == nil)
        model.selection.side = .from
        model.selectAsset(.bsv)
        #expect(model.selection.to == .usdt)
        model.selection.side = .to
        model.selectAsset(.trx)
        #expect(model.selection.from == .bsv)
        #expect(model.selection.to == .usdt)
        model.onDisappear()
    }

    @Test
    func oneWayPairLimitsSourceDestinationAndReverse() async {
        let service = BsvHomePortalSwapServiceMock(onPairs: { [.mock(direction: .usdtToTrx)] })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.selectPair(from: .usdt, to: .trx)
        #expect(model.assets(for: .from).isEmpty)
        model.selection.side = .from
        model.selectAsset(.usdt)
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .trx)
        #expect(model.assets(for: .to).isEmpty)
        #expect(!model.canReverse)
        model.reverse()
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .trx)
        model.amount = "2.5"
        await model.reloadQuote()
        #expect(await service.quoteInputs.first?.0 == .usdtToTrx)
        model.onDisappear()
    }

    @Test
    func failedPairsCannotQuoteOrSelectUnsupportedAssets() async {
        let service = BsvHomePortalSwapServiceMock(onPairs: {
            throw BsvHomePortalSwapError.Auth(error: .Api(code: "UNAVAILABLE", msg: "Unable to connect"))
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.selection.side = .from
        model.selectAsset(.trx)
        model.reverse()
        model.amount = "2.5"
        await model.reloadQuote()
        await model.createOrder()
        #expect(model.assets(for: .from).isEmpty)
        #expect(model.selection.from == .bsv)
        #expect(model.selection.to == .usdt)
        #expect(model.connection == .failed("Unable to connect"))
        #expect(await service.quoteInputs.isEmpty)
        #expect(await service.createOrderCount == 0)
        #expect(!model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func emptyPairsDisableNewExchange() async {
        let service = BsvHomePortalSwapServiceMock(onPairs: { [] })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "2.5"
        await model.reloadQuote()
        await model.createOrder()
        #expect(model.connection == .connected)
        #expect(model.assets(for: .from).isEmpty)
        #expect(model.assets(for: .to).isEmpty)
        #expect(!model.canReverse)
        #expect(await service.quoteInputs.isEmpty)
        #expect(await service.createOrderCount == 0)
        model.onDisappear()
    }

    @Test
    func refreshingPairsRemovesAndRestoresCurrentQuote() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "2.5"
        await model.reloadQuote()
        await service.setPairsResult(.success([.mock(direction: .usdtToTrx)]))
        await model.refresh()
        #expect(model.selection.from == .bsv)
        #expect(model.selection.to == .usdt)
        #expect(model.amount == "2.5")
        #expect(model.pair == nil)
        #expect(model.quote == nil)
        #expect(!model.canCreateOrder)
        #expect(model.assets(for: .from).isEmpty)
        #expect(await service.quoteInputs.count == 1)
        await service.setPairsResult(.success([.mock(), .mock(direction: .usdtToTrx)]))
        await model.retryQuote()
        #expect(model.assets(for: .from) == [.bsv])
        #expect(model.quote?.direction == .bsvToUsdt)
        #expect(model.canCreateOrder)
        #expect(await service.quoteInputs.count == 2)
        model.onDisappear()
    }

    @Test
    func failedCapabilityRefreshClearsQuoteAndCanRetry() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "2.5"
        await model.reloadQuote()
        await service.setPairsResult(.failure(.Auth(error: .Api(code: "UNAVAILABLE", msg: "Unable to connect"))))
        await model.refresh()
        #expect(model.connection == .failed("Unable to connect"))
        #expect(model.quote == nil)
        #expect(model.assets(for: .from).isEmpty)
        #expect(!model.canCreateOrder)
        await service.setPairsResult(.success([.mock(direction: .usdtToTrx), .mock()]))
        model.retryConnection()
        await model.load()
        model.selectPair(from: .usdt, to: .trx)
        model.amount = "2.5"
        await model.reloadQuote()
        #expect(model.quote?.direction == .usdtToTrx)
        #expect(model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func capabilityRefreshDiscardsInFlightQuote() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let service = BsvHomePortalSwapServiceMock(onQuote: { direction, _ in
            started.continuation.yield(())
            for await _ in response.stream {
                break
            }
            return .mock(direction: direction)
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "2.5"
        let task = Task { await model.reloadQuote() }
        for await _ in started.stream {
            break
        }
        await service.setPairsResult(.success([.mock(direction: .usdtToTrx)]))
        await model.refresh()
        response.continuation.finish()
        await task.value
        #expect(model.quote == nil)
        #expect(!model.isLoadingQuote)
        #expect(!model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func lateCapabilitiesCannotRestoreAfterLeaving() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: [BsvHomePortalSwapPair].self)
        let service = BsvHomePortalSwapServiceMock(onPairs: {
            started.continuation.yield(())
            for await pairs in response.stream {
                return pairs
            }
            return []
        }, onCancel: { response.continuation.finish() })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        response.continuation.yield([.mock()])
        await model.load()
        for await _ in started.stream {
            break
        }
        let task = Task { await model.refresh() }
        for await _ in started.stream {
            break
        }
        #expect(model.connection == .connecting)
        #expect(model.assets(for: .from).isEmpty)
        model.onDisappear()
        await task.value
        #expect(model.connection == .idle)
        #expect(model.pairs.isEmpty)
        #expect(model.quote == nil)
    }

    @Test
    func missingEntryPairKeepsEntryLabelsWithoutQuote() async {
        let service = BsvHomePortalSwapServiceMock(onPairs: { [.mock(direction: .usdtToTrx)] })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        model.selectEntryAsset(WowPayAsset.usdt.assetId)
        await model.load()
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .bsv)
        model.amount = "2.5"
        await model.reloadQuote()
        #expect(model.pair == nil)
        #expect(!model.isEntryAssetUnsupported)
        #expect(await service.quoteInputs.isEmpty)
        model.onDisappear()
    }

    @Test
    func loadsOnceAndRevalidatesAfterReturning() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        await model.load()
        #expect(model.connection == .connected)
        #expect(await service.pairsCount == 1)
        model.onDisappear()
        await model.load()
        #expect(model.connection == .connected)
        #expect(await service.pairsCount == 2)
        model.onDisappear()
    }

    @Test
    func cancelledAuthenticationRequiresExplicitRetry() async {
        let service = BsvHomePortalSwapServiceMock(onPairs: { throw BsvHomePortalSwapError.Auth(error: .Service(error: .Cancelled)) })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        #expect(model.connection == .cancelled)
        model.onDisappear()
        await model.load()
        #expect(await service.pairsCount == 1)
        model.retryConnection()
        await model.load()
        #expect(await service.pairsCount == 2)
        #expect(model.connection == .cancelled)
    }

    @Test
    func lateResponseCannotRestoreConnectionAfterLeaving() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let service = BsvHomePortalSwapServiceMock(
            onPairs: {
                started.continuation.yield(())
                for await _ in response.stream {
                    break
                }
                return [.mock()]
            },
            onCancel: { response.continuation.finish() },
        )
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        let task = Task { await model.load() }
        for await _ in started.stream {
            break
        }
        model.onDisappear()
        await task.value
        #expect(model.connection == .idle)
        #expect(model.pairs.isEmpty)
        #expect(model.orders.isEmpty)
    }

    @Test
    func obsoleteQuoteCannotReplaceCurrentInput() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let service = BsvHomePortalSwapServiceMock(onQuote: { direction, amount in
            if amount == "1" {
                started.continuation.yield(())
                for await _ in response.stream {
                    break
                }
                return .mock(quoteId: "obsolete", direction: direction)
            }
            return .mock(quoteId: "current", direction: direction)
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "1"
        let task = Task { await model.reloadQuote() }
        for await _ in started.stream {
            break
        }
        model.amount = "2"
        model.amountChanged()
        await model.reloadQuote()
        response.continuation.finish()
        await task.value
        #expect(model.quote?.quoteId == "current")
        #expect(!model.isLoadingQuote)
        model.onDisappear()
    }

    @Test
    func duplicateCreateCannotOpenSecondOrder() async {
        let started = AsyncStream.makeStream(of: Void.self)
        let response = AsyncStream.makeStream(of: Void.self)
        let service = BsvHomePortalSwapServiceMock(onCreateOrder: { _ in
            started.continuation.yield(())
            for await _ in response.stream {
                break
            }
            return .mock()
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "1"
        await model.reloadQuote()
        let task = Task { await model.createOrder() }
        for await _ in started.stream {
            break
        }
        await model.createOrder()
        #expect(await service.createOrderCount == 1)
        #expect(model.operation == .creating)
        response.continuation.finish()
        await task.value
        #expect(model.selectedOrder?.orderId == "order")
        #expect(model.quote == nil)
        #expect(model.operation == .idle)
        model.onDisappear()
    }

    @Test
    func providerFailureShowsMessageAndDisablesCreation() async {
        let service = BsvHomePortalSwapServiceMock(onQuote: { _, _ in
            throw BsvHomePortalSwapError.Auth(error: .Api(code: "SWAP_PROVIDER_UNAVAILABLE", msg: "Provider unavailable"))
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "10"
        await model.reloadQuote()
        #expect(model.quoteError == Localized.Info.NoQuote.description)
        #expect(model.quote == nil)
        #expect(!model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func providerQuoteTranslationKeyUsesLocalizedDescription() async {
        let service = BsvHomePortalSwapServiceMock(onQuote: { _, _ in
            throw BsvHomePortalSwapError.Auth(error: .Api(code: "SWAP_PROVIDER_UNAVAILABLE", msg: "RSP_MSG.SWAP_PROVIDER_UNAVAILABLE#I18N"))
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "1"
        await model.reloadQuote()
        #expect(model.quoteError == Localized.Info.NoQuote.description)
        #expect(model.quote == nil)
        #expect(!model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func expiredQuoteCannotEnableOrCreateAnOrder() async {
        let service = BsvHomePortalSwapServiceMock(onQuote: { direction, _ in .mock(direction: direction, expiresAt: 0) })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "1"
        await model.reloadQuote()
        #expect(model.quote == nil)
        #expect(model.quoteError != nil)
        #expect(!model.canCreateOrder)
        await model.createOrder()
        #expect(await service.createOrderCount == 0)
        model.onDisappear()
    }

    @Test
    func cachedOrdersRemainVisibleWhenRefreshFails() async {
        let cached = BsvHomePortalSwapOrder.mock(orderStatus: .refundPending, canPay: false)
        let service = BsvHomePortalSwapServiceMock(
            onOrders: { _, _, _ in throw BsvHomePortalSwapError.Auth(error: .Api(code: "UNAVAILABLE", msg: "Unable to update orders")) },
            cachedOrders: [cached],
        )
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        #expect(model.connection == .connected)
        #expect(model.orders == [cached])
        #expect(model.orders.first?.orderStatus == .refundPending)
        #expect(model.orderError == "\(Localized.wowpaySwapOrdersLoadFailed)\nUnable to update orders")
        #expect(!model.hasNoOrders)
        #expect(!model.orders[0].canPay)
        model.onDisappear()
    }

    @Test
    func failedOrdersDoNotShowEmptyResults() async {
        let service = BsvHomePortalSwapServiceMock(onOrders: { _, _, _ in
            throw BsvHomePortalSwapError.Auth(error: .InvalidResponse)
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        #expect(model.connection == .connected)
        #expect(model.orders.isEmpty)
        #expect(!model.hasNoOrders)
        #expect(model.orderError == "\(Localized.wowpaySwapOrdersLoadFailed)\n\(Localized.wowpaySwapInvalidResponse)")
        model.onDisappear()
    }

    @Test
    func emptyOrdersRequireSuccessfulLoading() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        #expect(!model.hasNoOrders)
        await model.load()
        #expect(model.hasNoOrders)
        model.onDisappear()
        #expect(!model.hasNoOrders)
    }

    @Test
    func retryOrdersClearsFailureAndShowsOrders() async {
        let service = BsvHomePortalSwapServiceMock(onOrders: { _, _, _ in
            throw BsvHomePortalSwapError.Auth(error: .InvalidResponse)
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        let order = BsvHomePortalSwapOrder.mock()
        await service.setOrdersResult(.success(BsvHomePortalSwapOrderPage(total: 1, records: [order])))
        await model.reloadOrders()
        #expect(model.orderError == nil)
        #expect(model.orders == [order])
        #expect(model.totalOrders == 1)
        #expect(!model.hasNoOrders)
        model.onDisappear()
    }

    @Test(arguments: [
        BsvHomePortalSwapError.InvalidResponse,
        .Auth(error: .InvalidResponse),
    ])
    func invalidResponseHasSpecificMessage(error: BsvHomePortalSwapError) async {
        let service = BsvHomePortalSwapServiceMock(onQuote: { _, _ in throw error })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "1"
        await model.reloadQuote()
        #expect(model.quoteError == Localized.wowpaySwapInvalidResponse)
        #expect(!model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func invalidSessionIsNotAnInvalidResponse() async {
        let service = BsvHomePortalSwapServiceMock(onQuote: { _, _ in
            throw BsvHomePortalSwapError.Auth(error: .InvalidSession)
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.amount = "1"
        await model.reloadQuote()
        #expect(model.quoteError == Localized.Errors.unknown)
        model.onDisappear()
    }

    @Test
    func loadingOrdersUpdatesRecoveredPendingState() async {
        let pending = AsyncStream.makeStream(of: Bool.self)
        pending.continuation.yield(true)
        pending.continuation.yield(false)
        pending.continuation.finish()
        let service = BsvHomePortalSwapServiceMock(
            onOrders: { _, _, _ in BsvHomePortalSwapOrderPage(total: 1, records: [.mock()]) },
            onHasPendingOrder: {
                for await value in pending.stream {
                    return value
                }
                throw BsvHomePortalSwapError.InvalidResponse
            },
        )
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        #expect(!model.hasPendingOrder)
        model.amount = "1"
        await model.reloadQuote()
        #expect(model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func loadingOrdersKeepsUnresolvedPendingState() async {
        let service = BsvHomePortalSwapServiceMock(
            onOrders: { _, _, _ in BsvHomePortalSwapOrderPage(total: 1, records: [.mock()]) },
            onHasPendingOrder: { true },
        )
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        #expect(model.hasPendingOrder)
        model.amount = "1"
        await model.reloadQuote()
        #expect(!model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func pendingStateFailureKeepsRecoveryAndShowsDetails() async {
        let pending = AsyncStream.makeStream(of: Result<Bool, BsvHomePortalSwapError>.self)
        pending.continuation.yield(.success(true))
        pending.continuation.yield(.failure(.Service(error: .Store(msg: "Unable to read order recovery"))))
        pending.continuation.finish()
        let service = BsvHomePortalSwapServiceMock(
            onOrders: { _, _, _ in BsvHomePortalSwapOrderPage(total: 1, records: [.mock()]) },
            onHasPendingOrder: {
                for await value in pending.stream {
                    return try value.get()
                }
                throw BsvHomePortalSwapError.InvalidResponse
            },
        )
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        #expect(model.hasPendingOrder)
        #expect(model.orders.count == 1)
        #expect(model.orderError == "\(Localized.wowpaySwapOrdersLoadFailed)\nUnable to read order recovery")
        #expect(!model.hasNoOrders)
        model.onDisappear()
    }

    @Test
    func loadingMoreOrdersUpdatesPendingState() async {
        let pending = AsyncStream.makeStream(of: Bool.self)
        for value in [true, true, false] {
            pending.continuation.yield(value)
        }
        pending.continuation.finish()
        let service = BsvHomePortalSwapServiceMock(
            onOrders: { _, page, _ in BsvHomePortalSwapOrderPage(total: 2, records: [.mock(orderId: "order-\(page)")]) },
            onHasPendingOrder: {
                for await value in pending.stream {
                    return value
                }
                throw BsvHomePortalSwapError.InvalidResponse
            },
        )
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        #expect(model.hasPendingOrder)
        await model.loadMoreOrders()
        #expect(!model.hasPendingOrder)
        #expect(model.orders.map(\.orderId) == ["order-1", "order-2"])
        #expect(!model.hasMoreOrders)
        model.onDisappear()
    }

    @Test
    func loadingMoreOrdersFailureKeepsLoadedOrders() async {
        let service = BsvHomePortalSwapServiceMock(onOrders: { _, page, _ in
            if page == 1 {
                return BsvHomePortalSwapOrderPage(total: 2, records: [.mock()])
            }
            throw BsvHomePortalSwapError.Auth(error: .InvalidResponse)
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        await model.loadMoreOrders()
        #expect(model.orders.count == 1)
        #expect(model.totalOrders == 2)
        #expect(model.hasMoreOrders)
        #expect(model.orderError == "\(Localized.wowpaySwapOrdersLoadFailed)\n\(Localized.wowpaySwapInvalidResponse)")
        #expect(!model.hasNoOrders)
        model.onDisappear()
    }

    @Test
    func cachedOrdersRemainVisibleWhenConnectingOffline() async {
        let cached = BsvHomePortalSwapOrder.mock(orderStatus: .refundPending, canPay: false)
        let service = BsvHomePortalSwapServiceMock(
            onPairs: { throw BsvHomePortalSwapError.Auth(error: .Api(code: "UNAVAILABLE", msg: "Unable to connect")) },
            cachedOrders: [cached],
        )
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        #expect(model.connection == .failed("Unable to connect"))
        #expect(model.orders == [cached])
        #expect(!model.orders[0].canPay)
        #expect(!model.canCreateOrder)
        model.onDisappear()
    }

    @Test
    func coreAssetMappingSelectsEntryDirection() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        model.selectedOrder = .mock()
        model.selectEntryAsset(WowPayAsset.usdt.assetId)
        await model.load()
        #expect(model.direction == .usdtToBsv)
        #expect(model.selection.from == .usdt)
        #expect(model.selection.to == .bsv)
        #expect(!model.isEntryAssetUnsupported)
        #expect(model.selectedOrder == nil)
        model.onDisappear()
    }

    @Test
    func unsupportedEntryAssetKeepsDefaultPairAvailable() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        model.selectEntryAsset(AssetId(chain: .ethereum))
        await model.load()

        #expect(model.isEntryAssetUnsupported)
        #expect(model.selection.from == .bsv)
        #expect(model.selection.to == .usdt)
        model.amount = "1"
        await model.reloadQuote()
        #expect(model.quote != nil)
        model.onDisappear()
    }

    @Test
    func paymentUsesCoreEligibilityAndPreservesFailureAfterRefresh() async {
        let service = BsvHomePortalSwapServiceMock()
        var requestedOrders: [String] = []
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { orderId, _ in
            requestedOrders.append(orderId)
            throw BsvHomePortalAuthError.Api(code: "PAYMENT_FAILED", msg: "Payment failed")
        })
        await model.load()
        await model.payOrder(.mock(canPay: false))
        #expect(requestedOrders.isEmpty)
        await model.payOrder(.mock())
        #expect(requestedOrders == ["order"])
        #expect(model.orderError == "Payment failed")
        #expect(model.orders.first?.orderStatus == .awaitingSignature)
        #expect(await service.refreshOrderCount == 1)
        model.onDisappear()
    }

    @Test
    func actionNavigationDoesNotCompleteVerificationLocally() async {
        let order = BsvHomePortalSwapOrder.mock(orderStatus: .onHold, verificationStatus: .required, canPay: false)
        let service = BsvHomePortalSwapServiceMock(onResolveAction: { _, _ in
            BsvHomePortalSwapActionNavigation(navigationUrl: "https://example.com/order", entryMode: .providerOrderPage, expiresAt: nil)
        })
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in })
        await model.load()
        model.selectedOrder = order
        await model.openAction(orderId: order.orderId, actionId: "verify")
        #expect(model.actionUrl?.absoluteString == "https://example.com/order")
        #expect(model.selectedOrder?.verificationStatus == .required)
        #expect(model.selectedOrder?.orderStatus == .onHold)
        model.onDisappear()
    }

    @Test
    func paymentCancellationDoesNotShowAnErrorOrRefresh() async {
        let service = BsvHomePortalSwapServiceMock()
        let model = WowPayPortalSwapViewModel(makeService: { service }, onPayment: { _, _ in throw CancellationError() })
        await model.load()
        await model.payOrder(.mock())
        #expect(model.orderError == nil)
        #expect(model.operation == .idle)
        #expect(await service.refreshOrderCount == 0)
        model.onDisappear()
    }
}
