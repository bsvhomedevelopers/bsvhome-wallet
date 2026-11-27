// Copyright (c). Gem Wallet. All rights reserved.

import Style
import SwiftUI

public protocol SelectableSheetViewable: SelectableListAdoptable {
    var title: String { get }
    var cancelButtonTitle: String { get }
    var clearButtonTitle: String { get }
    var doneButtonTitle: String { get }
    var confirmButtonTitle: String { get }
}

public struct SelectableSheet<ViewModel: SelectableSheetViewable, Content: View>: View {
    public typealias ListContent = (ViewModel.Item) -> Content
    public typealias FinishSelection = (SelectionResult<ViewModel.Item>) -> Void

    @Environment(\.dismiss) var dismiss

    @State private var model: ViewModel
    private let style: ListItemView.Style
    private let onFinishSelection: FinishSelection
    private let listContent: ListContent

    public init(
        model: ViewModel,
        style: ListItemView.Style = .standard,
        onFinishSelection: @escaping FinishSelection,
        listContent: @escaping ListContent,
    ) {
        _model = State(initialValue: model)
        self.style = style
        self.onFinishSelection = onFinishSelection
        self.listContent = listContent
    }

    public var body: some View {
        NavigationStack {
            SelectableListView(
                model: $model,
                onFinishSelection: { onFinish(items: $0, isConfirmed: false) },
                listContent: { item in
                    listContent(item)
                        .listRowBackground(style == .wowPay ? Colors.WowPay.surface : nil)
                },
            )
            .safeAreaButton {
                StateButton(
                    text: model.confirmButtonTitle,
                    type: style == .wowPay ? .wowPayPrimary() : .primary(.normal),
                    action: onConfirm,
                )
            }
            .contentMargins(.top, .scene.top, for: .scrollContent)
            .scrollContentBackground(style == .wowPay ? .hidden : .automatic)
            .background(style == .wowPay ? Colors.WowPay.background : Colors.grayBackground)
            .tint(style == .wowPay ? Colors.WowPay.brand : nil)
            .navigationTitle(model.title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                let cancelToolbarItem = {
                    ToolbarItem(placement: .navigationBarLeading) {
                        Button(model.cancelButtonTitle, action: onCancel)
                            .bold()
                    }
                }
                switch model.selectionType {
                case .multiSelection:
                    if model.selectedItems.isEmpty {
                        cancelToolbarItem()
                    } else {
                        ToolbarItem(placement: .navigationBarLeading) {
                            Button(model.clearButtonTitle, action: onReset)
                                .bold()
                        }
                    }
                    ToolbarItem(placement: .topBarTrailing) {
                        Button(model.doneButtonTitle) {
                            onDone()
                        }
                        .bold()
                    }
                case .navigationLink, .checkmark:
                    cancelToolbarItem()
                }
            }
        }
    }
}

// MARK: - Actions

extension SelectableSheet {
    private func onCancel() {
        dismiss()
    }

    private func onDone() {
        onFinish(items: Array(model.selectedItems), isConfirmed: false)
        dismiss()
    }

    private func onConfirm() {
        onFinish(items: Array(model.selectedItems), isConfirmed: true)
    }

    private func onReset() {
        model.reset()
    }

    private func onFinish(items: [ViewModel.Item], isConfirmed: Bool) {
        let value = SelectionResult(items: items, isConfirmed: isConfirmed)
        onFinishSelection(value)
    }
}
