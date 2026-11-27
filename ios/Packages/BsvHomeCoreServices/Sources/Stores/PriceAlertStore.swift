// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomePriceAlertStore
import struct BsvHomeCore.PriceAlert
import class BsvHomeCore.PriceAlertFormatter
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCorePriceAlertStore: BsvHomePriceAlertStore, @unchecked Sendable {
    private let store: PriceAlertStore

    public init(store: PriceAlertStore) {
        self.store = store
    }

    public func getPriceAlerts(assetId: String?) async throws -> [BsvHomeCore.PriceAlert] {
        let alerts = try assetId.map { try store.getPriceAlerts(for: $0) } ?? store.getPriceAlerts()
        return alerts.map { $0.map() }
    }

    public func updatePriceAlerts(alerts: [BsvHomeCore.PriceAlert], deleteIds: [String]) async throws {
        try store.diffPriceAlerts(
            deleteIds: deleteIds,
            alerts: alerts.map { (id: PriceAlertFormatter.shared.alertId(alert: $0), alert: $0.map()) },
        )
    }
}
