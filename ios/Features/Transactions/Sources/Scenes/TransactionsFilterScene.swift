// Copyright (c). Gem Wallet. All rights reserved.

import Components
import enum BsvHomeCore.BsvHomeTransactionFilter
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

public struct TransactionsFilterScene: View {
    @Environment(\.dismiss) private var dismiss
    @Binding private var model: TransactionsFilterViewModel

    public init(model: Binding<TransactionsFilterViewModel>) {
        _model = model
    }

    public var body: some View {
        List {
            SelectFilterView(
                typeModel: model.chainsFilter.typeModel,
                style: .wowPay,
                action: model.onSelectChainsFilter,
            )
            .wowPayListCard()
            SelectFilterView(
                typeModel: model.transactionTypesFilter.typeModel,
                style: .wowPay,
                action: model.onSelectTypesFilter,
            )
            .wowPayListCard()
        }
        .contentMargins(.top, Spacing.wowPay.page, for: .scrollContent)
        .contentMargins(.horizontal, Spacing.wowPay.page, for: .scrollContent)
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .wowPayBackground()
        .navigationTitle(model.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if model.isAnyFilterSpecified {
                ToolbarItem(placement: .cancellationAction) {
                    Button(model.clear, action: onSelectClear)
                        .bold()
                }
            }
            ToolbarItem(placement: .primaryAction) {
                Button(model.done, action: onSelectDone)
                    .bold()
            }
        }
        .sheet(isPresented: $model.isPresentingChains) {
            SelectableSheet(
                model: model.networksModel,
                style: .wowPay,
                onFinishSelection: onFinishSelection(value:),
                listContent: { chain in
                    let row = ChainViewModel(chain: chain)
                    return ListItemView(
                        title: row.title,
                        titleStyle: .wowPay.rowTitle,
                        imageStyle: .asset(assetImage: row.assetImage),
                    )
                },
            )
        }
        .sheet(isPresented: $model.isPresentingTypes) {
            SelectableSheet(
                model: model.typesModel,
                style: .wowPay,
                onFinishSelection: onFinishSelection(value:),
                listContent: {
                    ListItemView(title: TransactionFilterTypeViewModel(type: $0).title, titleStyle: .wowPay.rowTitle)
                },
            )
        }
    }
}

// MARK: - Actions

extension TransactionsFilterScene {
    private func onSelectClear() {
        model.chainsFilter.selectedChains = []
        model.transactionTypesFilter.selectedTypes = []
    }

    private func onSelectDone() {
        dismiss()
    }

    private func onFinishSelection(value: SelectionResult<Chain>) {
        model.chainsFilter.selectedChains = value.items
        if value.isConfirmed {
            dismiss()
        }
    }

    private func onFinishSelection(value: SelectionResult<BsvHomeTransactionFilter>) {
        model.transactionTypesFilter.selectedTypes = value.items
        if value.isConfirmed {
            dismiss()
        }
    }
}
