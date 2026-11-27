// Copyright (c). Gem Wallet. All rights reserved.

import protocol BsvHomeCore.BsvHomeFiatQuoteServiceProtocol
import Components
import BsvHomeCoreServices
import Foundation
import Localization
import Primitives
import PrimitivesComponents
import Store

@Observable
@MainActor
public final class FiatTransactionsViewModel {
    private let service: any BsvHomeFiatQuoteServiceProtocol
    let walletId: WalletId

    public let query: ObservableQuery<FiatTransactionsRequest>
    var transactions: [FiatTransactionAssetData] {
        query.value
    }

    var sections: [ListSection<FiatTransactionAssetData>] {
        DateSectionBuilder(items: transactions, dateKeyPath: \.createdAt).build()
    }

    init(walletId: WalletId, service: any BsvHomeFiatQuoteServiceProtocol) {
        self.walletId = walletId
        self.service = service
        query = ObservableQuery(FiatTransactionsRequest(walletId: walletId), initialValue: [])
    }

    var title: String {
        Localized.Activity.title
    }

    var emptyContentModel: EmptyContentTypeViewModel {
        EmptyContentTypeViewModel(type: .activity(isViewOnly: false))
    }

    func load() async {
        do {
            try await service.syncTransactions()
        } catch {
            debugLog("FiatTransactionsViewModel load error: \(error)")
        }
    }
}
