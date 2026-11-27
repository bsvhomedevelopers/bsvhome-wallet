// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import Localization
import SwiftUI

enum TabItem: Identifiable, CaseIterable {
    case home
    case wallet
    case swap
    case settings

    static let allCases: [Self] = [.home, .wallet, .settings]

    var title: String {
        switch self {
        case .home: Localized.wowpayHome
        case .wallet: Localized.Assets.title
        case .swap: Localized.Wallet.swap
        case .settings: Localized.wowpayProfile
        }
    }

    var image: Image {
        switch self {
        case .home: Image("wowpay-home")
        case .wallet: Image("wowpay-wallet-outline")
        case .swap: Image("wowpay-services")
        case .settings: Image("wowpay-profile")
        }
    }

    var id: Self {
        self
    }
}
