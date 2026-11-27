// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import InfoSheet
import BsvHomeCorePrimitives
import Primitives
import struct BsvHomeCore.BsvHomeTransferData

public enum AssetSheetType: Identifiable, Sendable {
    case info(InfoSheetType)
    case transfer(BsvHomeTransferData)
    case share
    case url(URL)

    public var id: String {
        switch self {
        case let .info(type): "info-\(type.id)"
        case let .transfer(data): "transfer-\(data.id)"
        case .share: "share"
        case let .url(url): "url-\(url)"
        }
    }
}
