// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeTransactionFilter
import Primitives

public enum TransactionsFilterType {
    case allTypes
    case type(name: BsvHomeTransactionFilter)
    case types(selected: [BsvHomeTransactionFilter])

    public init(selectedTypes: [BsvHomeTransactionFilter]) {
        switch selectedTypes.count {
        case 0: self = .allTypes
        case 1: self = .type(name: selectedTypes[0])
        default: self = .types(selected: selectedTypes)
        }
    }
}

extension TransactionsFilterType: Equatable {}
