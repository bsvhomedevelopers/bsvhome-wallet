// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeImage

public extension BsvHomeImage {
    var imageURL: URL? {
        URL(string: url())
    }
}
