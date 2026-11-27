// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Formatters
import struct BsvHomeCore.BsvHomeAssetRow
import Primitives
import PrimitivesComponents
import SwiftUI

struct ListAssetItemSelectionView: View {
    private let assetData: AssetData
    private let currencyCode: String
    private let row: BsvHomeAssetRow
    private let action: (ListAssetItemAction, AssetData) -> Void

    init(
        assetData: AssetData,
        currencyCode: String,
        row: BsvHomeAssetRow,
        action: @escaping (ListAssetItemAction, AssetData) -> Void,
    ) {
        self.assetData = assetData
        self.currencyCode = currencyCode
        self.row = row
        self.action = action
    }

    var body: some View {
        ListAssetItemView(
            model: ListAssetItemViewModel(
                showBalancePrivacy: .constant(false),
                assetDataModel: AssetDataViewModel(
                    assetData: assetData,
                formatter: ValueFormatter.short,
                    currencyCode: currencyCode,
                ),
                row: row,
                action: {
                    action($0, assetData)
                },
            ),
        )
    }
}
