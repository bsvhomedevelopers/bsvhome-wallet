// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeTransactionStateServiceProtocol
import protocol BsvHomeCore.BsvHomeTransactionStatusService
import struct BsvHomeCore.Transaction
import Primitives

public final class BsvHomeCoreTransactionStatusService: BsvHomeTransactionStatusService, Sendable {
    private let service: any BsvHomeTransactionStateServiceProtocol

    public init(service: any BsvHomeTransactionStateServiceProtocol) {
        self.service = service
    }

    public func track(walletId: String, transactions: [BsvHomeCore.Transaction]) {
        let generation = service.trackingGeneration()
        Task {
            do {
                try await service.trackWithGeneration(walletId: walletId, transactions: transactions, generation: generation)
            } catch {
                debugLog("BsvHomeCoreTransactionStatusService: failed for \(walletId): \(error)")
            }
        }
    }
}
