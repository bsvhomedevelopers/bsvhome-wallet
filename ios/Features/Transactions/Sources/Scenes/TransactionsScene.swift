// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI

public struct TransactionsScene: View {
    private var model: TransactionsViewModel

    public init(model: TransactionsViewModel) {
        self.model = model
    }

    @Environment(\.connectionStatus) private var connectionStatus

    public var body: some View {
        VStack {
            List {
                if model.state.isLoading {
                    LoadingView()
                        .frame(maxWidth: .infinity)
                        .cleanListRow()
                } else if model.state.isError {
                    StateEmptyView(title: model.sections.isEmpty ? Localized.Errors.errorOccurred : Localized.Errors.balancesActivityOutdated) {
                        Button(Localized.Common.tryAgain) { Task { await model.load() } }
                            .buttonStyle(.wowPaySecondary())
                    }
                    .cleanListRow()
                    .accessibilityIdentifier("transactionsRefreshError")
                }
                TransactionsList(sections: model.sections, currency: model.currency, style: .wowPay)
            }
            .listStyle(.insetGrouped)
            .contentMargins(.horizontal, Spacing.wowPay.page, for: .scrollContent)
            .listSectionSpacing(.custom(Spacing.wowPay.card))
            .scrollContentBackground(.hidden)
            .refreshableTimer(every: connectionStatus.refreshInterval(for: .wallet)) { _ in
                await model.load()
            }
        }
        .wowPayBackground(.solid)
        .overlay {
            if case .data = model.state, model.sections.isEmpty {
                EmptyContentView(model: model.emptyContentModel, style: .wowPay)
                    .padding(.horizontal, .medium)
            }
        }
        .task { await model.load() }
    }
}
