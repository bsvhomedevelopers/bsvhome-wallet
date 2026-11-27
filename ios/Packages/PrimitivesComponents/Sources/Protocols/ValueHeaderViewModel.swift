// Copyright (c). Gem Wallet. All rights reserved.

import Components
import SwiftUI

public protocol ValueHeaderViewModel {
    var isWatchWallet: Bool { get }
    var assetImage: AssetImage? { get }
    var title: String { get }
    var amountTitle: String { get }
    var amountSymbol: String? { get }
    var subtitle: String? { get }
    var subtitleImage: Image? { get }
    var subtitleColor: Color { get }
    var buttons: [HeaderButton] { get }
}

public extension ValueHeaderViewModel {
    var amountTitle: String { title }
    var amountSymbol: String? { nil }

    var subtitleImage: Image? {
        nil
    }
}
