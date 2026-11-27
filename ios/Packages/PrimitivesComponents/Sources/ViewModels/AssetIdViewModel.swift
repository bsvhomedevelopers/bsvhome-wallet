// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import class BsvHomeCore.BsvHomeAssetConfigService
import struct BsvHomeCore.BsvHomeAssetIcon
import enum BsvHomeCore.BsvHomeAssetIconImage
import BsvHomeCorePrimitives
import Primitives
import SwiftUI


private final class AssetIconCache: @unchecked Sendable {
    static let shared = AssetIconCache()

    private var icons: [String: BsvHomeAssetIcon] = [:]
    private let lock = NSLock()

    func icon(for identifier: String) -> BsvHomeAssetIcon {
        lock.withLock {
            if let cached = icons[identifier] {
                return cached
            }
            let icon = BsvHomeAssetConfigService.shared.assetIcon(assetId: identifier)
            icons[identifier] = icon
            return icon
        }
    }
}

public struct AssetIdViewModel: Sendable {
    private let assetId: AssetId

    public init(assetId: AssetId) {
        self.assetId = assetId
    }

    public var networkAssetImage: AssetImage {
        AssetImage(
            type: .text(.empty),
            imageURL: .none,
            placeholder: ChainImage(chain: assetId.chain).image,
            chainPlaceholder: .none,
        )
    }

    public var assetImage: AssetImage {
        let icon = AssetIconCache.shared.icon(for: assetId.identifier)
        let (imageURL, placeholder): (URL?, Image?) = switch icon.image {
        case let .local(chain): (.none, ChainImage(chain: Chain(core: chain)).image)
        case let .localToken(token): (.none, TokenImage(token: token).image)
        case let .remote(url): (URL(string: url), .none)
        }
        return AssetImage(
            type: .text(assetId.assetType?.rawValue ?? .empty),
            imageURL: imageURL,
            placeholder: placeholder,
            chainPlaceholder: icon.badge.map { ChainImage(chain: Chain(core: $0)).image },
        )
    }
}
