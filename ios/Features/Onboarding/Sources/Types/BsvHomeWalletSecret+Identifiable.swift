// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeWalletSecret

extension BsvHomeWalletSecret: @retroactive Identifiable {
    public var id: String {
        switch self {
        case .words: "words"
        case .privateKey: "privateKey"
        case .recovery: "recovery"
        }
    }
}
