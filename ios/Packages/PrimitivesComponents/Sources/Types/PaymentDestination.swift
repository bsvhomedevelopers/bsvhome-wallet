// Copyright (c). Gem Wallet. All rights reserved.

import BsvHomeCorePrimitives
import Primitives
import struct BsvHomeCore.BsvHomeTransferData

public enum PaymentDestination: Identifiable, Sendable {
    case confirm(BsvHomeTransferData)
    case recipient(SelectedAssetInput)
    case selectAsset(SelectAssetType, chains: [Chain])

    public var id: String {
        switch self {
        case let .confirm(data): "confirm-\(data.id)"
        case let .recipient(input): "recipient-\(input.id)"
        case let .selectAsset(type, _): "selectAsset-\(type.id)"
        }
    }
}
