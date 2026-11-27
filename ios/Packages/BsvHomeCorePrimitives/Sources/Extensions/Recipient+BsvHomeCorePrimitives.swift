// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeRecipient

extension BsvHomeRecipient: @retroactive Identifiable {
    public var id: String {
        identifier()
    }
}
