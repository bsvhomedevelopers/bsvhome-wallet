// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeTransactionFilter

extension BsvHomeTransactionFilter: @retroactive Identifiable {
    public var id: Self { self }
}
