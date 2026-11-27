// Copyright (c). Gem Wallet. All rights reserved.

import SwiftUI
import WidgetKit

@main
struct BsvHomePriceWidgetBundle: WidgetBundle {
    var body: some Widget {
        SmallPriceWidget()
        MediumPriceWidget()
    }
}
