// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeAssetRow

public extension BsvHomeAssetRow {
    static let wallet = BsvHomeAssetRow(title: .asset, showsSymbol: false, subtitle: .price, trailing: .balance)
}
