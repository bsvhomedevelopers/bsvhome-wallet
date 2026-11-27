// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Formatters
import Foundation
import enum BsvHomeCore.BsvHomeAssetMarketRow
import struct BsvHomeCore.BsvHomeChart
import enum BsvHomeCore.BsvHomeChartSection
import protocol BsvHomeCore.BsvHomeChartServiceProtocol
import func BsvHomeCore.priceChartData
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import InfoSheet
import Localization
import Primitives
import PrimitivesComponents
import Store
import SwiftUI

@MainActor
@Observable
public final class ChartSceneViewModel: ChartListViewable {
    private let service: any BsvHomeChartServiceProtocol
    private let preferences: ObservablePreferences

    var currencyCode: String {
        preferences.currency.rawValue
    }

    let walletId: WalletId
    let assetModel: AssetViewModel

    private var chart: StateViewType<BsvHomeChart> = .loading
    private var chartCurrencyCode: String?
    private var loadRevision = 0
    public var selectedPeriod: ChartPeriod {
        didSet { try? service.setChartPeriod(period: selectedPeriod.map()) }
    }

    public let priceQuery: ObservableQuery<PriceRequest>
    var priceData: PriceData? {
        priceQuery.value
    }

    var isPresentingInfoSheet: InfoSheetType?
    var isPresentingToastMessage: ToastMessage?
    private let onSetPriceAlert: (Asset) -> Void

    var title: String {
        assetModel.name
    }

    var asset: Asset {
        assetModel.asset
    }

    public var chartState: StateViewType<ChartValuesViewModel> {
        guard chartCurrencyCode == currencyCode else { return .loading }
        return chart.flatMap { chartValuesViewModel(from: $0).map { .data($0) } ?? .noData }
    }

    var sections: [BsvHomeChartSection] {
        guard let priceData else { return [] }
        return service.sections(
            asset: priceData.asset.map(),
            price: priceData.price?.price,
            market: priceData.market?.map(),
            priceAlerts: priceData.priceAlerts.map { $0.map() },
            links: priceData.links.map { $0.map() },
        )
    }

    public init(
        service: any BsvHomeChartServiceProtocol,
        preferences: ObservablePreferences,
        assetModel: AssetViewModel,
        walletId: WalletId,
        onSetPriceAlert: @escaping (Asset) -> Void,
    ) {
        self.service = service
        self.preferences = preferences
        self.assetModel = assetModel
        self.walletId = walletId
        selectedPeriod = service.chartPeriod().map()
        priceQuery = ObservableQuery(PriceRequest(assetId: assetModel.asset.id), initialValue: .with(asset: assetModel.asset))
        self.onSetPriceAlert = onSetPriceAlert
    }

    func marketValues(_ rows: [BsvHomeAssetMarketRow]) -> [MarketValueViewModel] {
        AssetDetailsInfoViewModel(asset: asset, currency: currencyCode).marketValues(rows)
    }

    private func chartValuesViewModel(from chart: BsvHomeChart) -> ChartValuesViewModel? {
        guard let chartData = priceChartData(chart: chart) else {
            return nil
        }
        return ChartValuesViewModel(period: selectedPeriod, chartData: chartData, formatter: CurrencyFormatter(currencyCode: currencyCode))
    }
}

// MARK: - Business Logic

public extension ChartSceneViewModel {
    func load() async {
        let currency = currencyCode
        let period = selectedPeriod
        loadRevision &+= 1
        let revision = loadRevision
        if chart.value == nil || chartCurrencyCode != currency {
            chart = .loading
        }
        chartCurrencyCode = currency
        do {
            let chart = try await service.syncCharts(assetId: assetModel.asset.id.identifier, period: period.map())
            guard !Task.isCancelled, revision == loadRevision, currencyCode == currency, selectedPeriod == period else { return }
            self.chart = .data(chart)
            if priceData?.priceAlerts.isNotEmpty == true {
                Task {
                    do {
                        try await service.syncPriceAlerts(assetId: assetModel.asset.id.identifier)
                    } catch {
                        debugLog("chart scene: price alerts update error \(error)")
                    }
                }
            }
        } catch {
            guard !Task.isCancelled, revision == loadRevision, currencyCode == currency, selectedPeriod == period else { return }
            chart.setError(error)
            if asset.id.chain == .bitcoinSv {
                isPresentingToastMessage = .error(error.localizedDescription)
            }
        }
    }

    func onSelectSetPriceAlerts() {
        onSetPriceAlert(assetModel.asset)
    }

    internal func onSelectInfoSheet(_ type: InfoSheetType) {
        isPresentingInfoSheet = type
    }
}
