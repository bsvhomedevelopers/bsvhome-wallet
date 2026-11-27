// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import enum BsvHomeCore.BsvHomeTransactionHeader
import PrimitivesComponents

struct TransactionHeaderViewModel {
    private let header: BsvHomeTransactionHeader
    private let currency: String

    init(header: BsvHomeTransactionHeader, currency: String) {
        self.header = header
        self.currency = currency
    }

    var headerType: TransactionHeaderType {
        header.headerType(currency: currency)
    }

    var showClearHeader: Bool {
        switch headerType {
        case .amount, .nft, .asset, .assetValue: true
        case .swap: false
        }
    }
}

extension TransactionHeaderViewModel: ItemModelProvidable {
    var itemModel: TransactionItemModel {
        .header(
            TransactionHeaderItemModel(
                headerType: headerType,
                showClearHeader: showClearHeader,
            ),
        )
    }
}
