// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeRecipient
import Primitives

public enum AddContactType: Hashable, Sendable {
    case new(BsvHomeRecipient, chain: Chain)
    case existing(BsvHomeRecipient, chain: Chain)

    public var id: String {
        switch self {
        case let .new(recipient, chain): "new-\(chain.rawValue)-\(recipient.identifier())"
        case let .existing(recipient, chain): "existing-\(chain.rawValue)-\(recipient.identifier())"
        }
    }
}
