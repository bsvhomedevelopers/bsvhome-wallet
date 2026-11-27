// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import protocol BsvHomeCore.BsvHomeTransactionsServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import Primitives
import PrimitivesComponents
import Store

@Observable
@MainActor
public final class TransactionsViewModel {
    private let service: any BsvHomeTransactionsServiceProtocol
    private let type: TransactionsRequestType

    public let wallet: Wallet

    public var sections: [ListSection<TransactionViewModel>] {
        filterModel.query.value
    }

    public var filterModel: TransactionsFilterViewModel

    public var isPresentingSheet: TransactionsSheetType?
    public var isPresentingToastMessage: ToastMessage?
    private(set) var state: StateViewType<Void> = .loading
    private var isLoading = false

    public init(
        service: any BsvHomeTransactionsServiceProtocol,
        wallet: Wallet,
        type: TransactionsRequestType,
    ) {
        self.service = service
        self.type = type
        self.wallet = wallet
        filterModel = TransactionsFilterViewModel(wallet: wallet, chains: Self.filterChains(service, wallet: wallet), type: type)
    }

    private static func filterChains(_ service: any BsvHomeTransactionsServiceProtocol, wallet: Wallet) -> [Chain] {
        service.filterChains(wallet: wallet.map()).map { Chain(core: $0) }
    }

    public var title: String {
        Localized.Activity.title
    }

    public var walletId: WalletId {
        wallet.id
    }

    public var currency: String {
        service.getCurrency()
    }

    public var emptyContentModel: EmptyContentTypeViewModel {
        if !filterModel.isAnyFilterSpecified {
            EmptyContentTypeViewModel(type: .activity(receive: onSelectReceive, buy: onSelectBuy, isViewOnly: wallet.isViewOnly))
        } else {
            EmptyContentTypeViewModel(type: .search(type: .activity, action: onSelectCleanFilters))
        }
    }
}

// MARK: - Business Logic

public extension TransactionsViewModel {
    func onSelectFilterButton() {
        isPresentingSheet = .filter
    }

    func load() async {
        guard !isLoading else { return }
        isLoading = true
        state = .loading
        defer { isLoading = false }
        do {
            try Task.checkCancellation()
            let assetId: String? = if case let .asset(assetId) = type {
                assetId.identifier
            } else {
                nil
            }
            try await service.syncWallet(walletId: wallet.id.id, assetId: assetId)
            try Task.checkCancellation()
            state = .data(())
        } catch {
            state = .error(error)
        }
    }
}

// MARK: - Private

extension TransactionsViewModel {
    private func onSelectCleanFilters() {
        filterModel = TransactionsFilterViewModel(wallet: wallet, chains: Self.filterChains(service, wallet: wallet), type: type)
    }

    private func onSelectReceive() {
        isPresentingSheet = .selectAsset(.receive(.asset))
    }

    private func onSelectBuy() {
        isPresentingSheet = .selectAsset(.buy)
    }
}
