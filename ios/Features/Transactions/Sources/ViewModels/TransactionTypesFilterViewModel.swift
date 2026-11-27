// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeTransactionFilter
import func BsvHomeCore.transactionFilters
import Primitives

public struct TransactionTypesFilterViewModel: Equatable {
    public let allTransactionsTypes: [BsvHomeTransactionFilter]
    public var selectedTypes: [BsvHomeTransactionFilter]

    public init() {
        allTransactionsTypes = transactionFilters()
        selectedTypes = []
    }

    public var requestFilters: [TransactionType] {
        selectedTypes.flatMap { $0.transactionTypes().map { $0.map() } }
    }

    public var typeModel: TransactionsFilterTypeViewModel {
        TransactionsFilterTypeViewModel(
            type: TransactionsFilterType(selectedTypes: selectedTypes),
        )
    }

    public var isAnySelected: Bool {
        !selectedTypes.isEmpty
    }

    public var isEmpty: Bool {
        allTransactionsTypes.isEmpty
    }
}
