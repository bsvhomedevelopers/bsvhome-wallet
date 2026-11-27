// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeApplicationMetadataService
import Primitives

public extension Primitives.ApplicationMetadata {
    var iconURL: URL? {
        BsvHomeApplicationMetadataService.shared.iconUrl(metadata: map()).flatMap(URL.init(string:))
    }

    var shortName: String {
        BsvHomeApplicationMetadataService.shared.shortName(metadata: map())
    }

    var host: String {
        BsvHomeApplicationMetadataService.shared.host(metadata: map())
    }
}
