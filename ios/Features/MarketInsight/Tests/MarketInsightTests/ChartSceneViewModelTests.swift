import Foundation
import BsvHomeCore
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
@testable import MarketInsight
import Primitives
import PrimitivesComponents
import PrimitivesTestKit
import Testing

@MainActor
struct ChartSceneViewModelTests {
    @Test
    func currencyChangeHidesThePreviousChart() async throws {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let observable = ObservablePreferences(preferencesService: preferences)
        let model = model(service: ChartServiceMock { chart(value: 100) }, preferences: observable)
        await model.load()
        #expect(model.chartState.value?.values.charts.last?.value == 100)

        try preferences.setCurrencyValue(.eur)
        observable.reload()

        #expect(model.currencyCode == "EUR")
        #expect(model.chartState.isLoading)
        #expect(model.chartState.value == nil)
    }

    @Test
    func oldCurrencyCompletionCannotReplaceTheCurrentChart() async throws {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let observable = ObservablePreferences(preferencesService: preferences)
        let pending = AsyncStream<CheckedContinuation<BsvHomeChart, any Error>>.makeStream()
        let model = model(service: ChartServiceMock {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }, preferences: observable)
        var requests = pending.stream.makeAsyncIterator()
        let oldLoad = Task { await model.load() }
        let oldCompletion = try #require(await requests.next())

        try preferences.setCurrencyValue(.eur)
        observable.reload()
        let currentLoad = Task { await model.load() }
        let currentCompletion = try #require(await requests.next())
        currentCompletion.resume(returning: chart(value: 90))
        await currentLoad.value
        #expect(model.chartState.value?.values.charts.last?.value == 90)

        oldCompletion.resume(returning: chart(value: 100))
        await oldLoad.value

        #expect(model.chartState.value?.values.charts.last?.value == 90)
        #expect(model.currencyCode == "EUR")
    }

    @Test
    func oldCurrencyFailureCannotReplaceTheCurrentChart() async throws {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let observable = ObservablePreferences(preferencesService: preferences)
        let pending = AsyncStream<CheckedContinuation<BsvHomeChart, any Error>>.makeStream()
        let model = model(service: ChartServiceMock {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }, preferences: observable)
        var requests = pending.stream.makeAsyncIterator()
        let oldLoad = Task { await model.load() }
        let oldCompletion = try #require(await requests.next())

        try preferences.setCurrencyValue(.eur)
        observable.reload()
        let currentLoad = Task { await model.load() }
        let currentCompletion = try #require(await requests.next())
        currentCompletion.resume(returning: chart(value: 90))
        await currentLoad.value
        oldCompletion.resume(throwing: AnyError("old currency request failed"))
        await oldLoad.value

        #expect(model.chartState.value?.values.charts.last?.value == 90)
        #expect(!model.chartState.isError)
        #expect(model.isPresentingToastMessage == nil)
    }

    @Test
    func cancelledCompletionCannotPublishChartData() async throws {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let observable = ObservablePreferences(preferencesService: preferences)
        let pending = AsyncStream<CheckedContinuation<BsvHomeChart, any Error>>.makeStream()
        let model = model(service: ChartServiceMock {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }, preferences: observable)
        var requests = pending.stream.makeAsyncIterator()
        let load = Task { await model.load() }
        let completion = try #require(await requests.next())
        load.cancel()
        completion.resume(returning: chart(value: 100))
        await load.value

        #expect(model.chartState.isLoading)
        #expect(model.chartState.value == nil)
    }

    private func model(service: ChartServiceMock, preferences: ObservablePreferences) -> ChartSceneViewModel {
        ChartSceneViewModel(
            service: service,
            preferences: preferences,
            assetModel: AssetViewModel(asset: Primitives.Chain.bitcoinSv.asset),
            walletId: .mock(),
            onSetPriceAlert: { _ in },
        )
    }
}

private final class ChartServiceMock: BsvHomeChartServiceProtocol, Sendable {
    private let response: @Sendable () async throws -> BsvHomeChart

    init(response: @escaping @Sendable () async throws -> BsvHomeChart) {
        self.response = response
    }

    func chartPeriod() -> BsvHomeCore.ChartPeriod {
        .day
    }

    func getCurrency() -> BsvHomeCore.Currency {
        "USD"
    }

    func setChartPeriod(period _: BsvHomeCore.ChartPeriod) throws {}
    func syncPriceAlerts(assetId _: BsvHomeCore.AssetId) async throws {}
    func sections(asset _: BsvHomeCore.Asset, price _: Double?, market _: BsvHomeCore.AssetMarket?, priceAlerts _: [BsvHomeCore.PriceAlert], links _: [BsvHomeCore.AssetLink]) -> [BsvHomeChartSection] {
        []
    }

    func syncCharts(assetId _: BsvHomeCore.AssetId, period _: BsvHomeCore.ChartPeriod) async throws -> BsvHomeChart {
        try await response()
    }
}

private func chart(value: Double) -> BsvHomeChart {
    BsvHomeChart(
        values: [
            BsvHomeCore.ChartDateValue(date: Date(timeIntervalSince1970: 1_700_000_000), value: value - 1),
            BsvHomeCore.ChartDateValue(date: Date(timeIntervalSince1970: 1_700_003_600), value: value),
        ],
        baseValue: value - 1,
        current: nil,
    )
}
