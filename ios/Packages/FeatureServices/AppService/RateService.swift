// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import Primitives
import protocol BsvHomeCore.BsvHomePreferencesServiceProtocol
import StoreKit

public struct RateService: Sendable {
    private let preferencesService: any BsvHomePreferencesServiceProtocol

    public init(preferencesService: any BsvHomePreferencesServiceProtocol) {
        self.preferencesService = preferencesService
    }

    public func perform() {
        #if targetEnvironment(simulator)
        #else
            guard preferencesService.shouldRequestReview() else { return }
            Task { @MainActor in
                if rate() {
                    do {
                        try preferencesService.setRateApplicationShown()
                    } catch {
                        debugLog("RateService: review preference write failed: \(error)")
                    }
                }
            }
        #endif
    }

    @MainActor
    @discardableResult
    private func rate() -> Bool {
        guard let scene = UIApplication.shared
            .connectedScenes
            .first(where: { $0.activationState == .foregroundActive }) as? UIWindowScene
        else { return false }
        AppStore.requestReview(in: scene)
        return true
    }
}
