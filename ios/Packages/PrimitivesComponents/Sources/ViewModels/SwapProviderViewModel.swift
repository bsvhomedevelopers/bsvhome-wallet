// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import struct BsvHomeCore.SwapProviderData
import BsvHomeCorePrimitives
import Primitives

public struct SwapProviderViewModel {
    private let providerData: BsvHomeCore.SwapProviderData

    public init(providerData: BsvHomeCore.SwapProviderData) {
        self.providerData = providerData
    }

    public var providerText: String {
        providerData.name
    }

    public var providerImage: AssetImage {
        AssetImage(
            imageURL: .none,
            placeholder: providerData.provider.map().image,
            chainPlaceholder: .none,
        )
    }
}
