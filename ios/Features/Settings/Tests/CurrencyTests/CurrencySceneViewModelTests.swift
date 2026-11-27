// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomePreferencesService
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import Observation
import Primitives
@testable import Settings
import Testing

@MainActor
struct CurrencySceneViewModelTests {
    @Test
    func uSDCurrencyValue() {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let viewModel = CurrencySceneViewModel(preferences: ObservablePreferences(preferencesService: preferences), service: BsvHomeCurrencyServiceMock(preferences: preferences))

        #expect(viewModel.selectedCurrencyValue == "🇺🇸 USD")
    }

    @Test
    func gBPCurrencyValue() throws {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        try preferences.setCurrency(currency: Currency.gbp.rawValue)
        let viewModel = CurrencySceneViewModel(preferences: ObservablePreferences(preferencesService: preferences), service: BsvHomeCurrencyServiceMock(flag: "🇬🇧", preferences: preferences))
        #expect(viewModel.selectedCurrencyValue == "🇬🇧 GBP")
    }

    @Test
    func setNewCurrency() async throws {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let observablePreferences = ObservablePreferences(preferencesService: preferences)
        let service = BsvHomeCurrencyServiceMock(preferences: preferences)
        let viewModel = CurrencySceneViewModel(preferences: observablePreferences, service: service)

        try await confirmation(expectedCount: 1) { changed in
            withObservationTracking {
                _ = observablePreferences.currency
            } onChange: {
                changed()
            }
            try await viewModel.setCurrency(.ars)
        }

        #expect(service.setCurrencies == [Currency.ars.rawValue])
        #expect(observablePreferences.currency == .ars)
        #expect(observablePreferences.currency == viewModel.currency)
    }

    @Test
    func aFailedChangeLeavesTheStoredCurrency() async {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        let observablePreferences = ObservablePreferences(preferencesService: preferences)
        let viewModel = CurrencySceneViewModel(preferences: observablePreferences, service: BsvHomeCurrencyServiceMock(error: AnyError("offline"), preferences: preferences))

        await #expect(throws: (any Error).self) { try await viewModel.setCurrency(.ars) }
        #expect(observablePreferences.currency == .usd)
    }
}
