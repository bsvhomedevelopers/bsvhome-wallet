// Copyright (c). Gem Wallet. All rights reserved.

import Primitives
import SwiftUI
import WidgetKit

struct PriceWidgetProvider: TimelineProvider {
    func placeholder(in context: Context) -> PriceWidgetEntry {
        PriceWidgetEntry.placeholder(widgetFamily: context.family)
    }

    func getSnapshot(in context: Context, completion: @escaping @Sendable (PriceWidgetEntry) -> Void) {
        let widgetFamily = context.family
        if context.isPreview {
            completion(PriceWidgetEntry.placeholder(widgetFamily: widgetFamily))
            return
        }
        guard DevicePurposeStore.appGroup.permitsNetworkAccess else {
            completion(PriceWidgetEntry(date: Date(), coinPrices: [], widgetFamily: widgetFamily))
            return
        }
        Task.detached {
            let widgetPriceService = WidgetPriceService()
            let entry = await widgetPriceService.fetchTopCoinPrices(widgetFamily: widgetFamily)
            completion(entry)
        }
    }

    func getTimeline(in context: Context, completion: @escaping @Sendable (Timeline<PriceWidgetEntry>) -> Void) {
        let widgetFamily = context.family
        let currentDate = Date()

        let purpose = try? DevicePurposeStore.appGroup.read()
        guard purpose == .daily else {
            let entry = PriceWidgetEntry(date: currentDate, coinPrices: [], widgetFamily: widgetFamily)
            let policy: TimelineReloadPolicy = purpose == .coldWallet ? .never : .after(currentDate.addingTimeInterval(15 * 60))
            completion(Timeline(entries: [entry], policy: policy))
            return
        }
        Task.detached {
            let widgetPriceService = WidgetPriceService()
            let entry = await widgetPriceService.fetchTopCoinPrices(widgetFamily: widgetFamily)
            let nextUpdateDate = Calendar.current.date(byAdding: .minute, value: 1, to: currentDate)!
            let timeline = Timeline(entries: [entry], policy: .after(nextUpdateDate))
            completion(timeline)
        }
    }
}
