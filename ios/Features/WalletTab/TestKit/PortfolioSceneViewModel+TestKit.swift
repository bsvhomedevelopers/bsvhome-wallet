// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomePortfolioServiceProtocol
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesTestKit
import WalletTab

public extension PortfolioSceneViewModel {
    @MainActor
    static func mock(
        wallet: Wallet = .mock(),
        service: any BsvHomePortfolioServiceProtocol = BsvHomePortfolioServiceMock(),
        preferences: ObservablePreferences = .mock(),
        defaultType: PortfolioType = .wallet,
    ) -> PortfolioSceneViewModel {
        PortfolioSceneViewModel(
            wallet: wallet,
            service: service,
            preferences: preferences,
            defaultType: defaultType,
        )
    }
}
