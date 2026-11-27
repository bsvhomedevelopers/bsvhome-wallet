// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeLocalTokenIcon
import Style
import SwiftUI

public struct TokenImage: Sendable {
    private let token: BsvHomeLocalTokenIcon

    public init(token: BsvHomeLocalTokenIcon) {
        self.token = token
    }

    public var image: Image {
        switch token {
        case .usdt: Images.Tokens.usdt
        case .usdc: Images.Tokens.usdc
        }
    }
}
