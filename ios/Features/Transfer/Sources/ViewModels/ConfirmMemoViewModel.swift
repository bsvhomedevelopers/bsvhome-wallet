// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeTransferData
import Components
import Primitives
import PrimitivesComponents

struct ConfirmMemoViewModel {
    private let transfer: BsvHomeTransferData

    init(transfer: BsvHomeTransferData) {
        self.transfer = transfer
    }
}

// MARK: - ItemModelProvidable

extension ConfirmMemoViewModel: ItemModelProvidable {
    var itemModel: ConfirmTransferItemModel {
        guard transfer.showsMemo() else { return .empty }
        return .memo(MemoViewModel(memo: transfer.recipient.memo).listItemModel)
    }
}
