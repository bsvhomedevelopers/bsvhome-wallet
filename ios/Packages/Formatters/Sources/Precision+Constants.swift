// Copyright (c). Gem Wallet. All rights reserved.

import Foundation

extension NumberFormatStyleConfiguration.Precision {
    static let full = fractionLength(0 ... 32)
    static let minimumFourPlaces = fractionLength(4 ... 324)
}
