// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Primitives
import Style
import SwiftUI

public struct CurrencyScene: View {
    @Environment(\.dismiss) private var dismiss
    @State private var model: CurrencySceneViewModel
    @State private var isChangingCurrency = false
    @State private var isPresentingAlertMessage: AlertMessage?

    public init(model: CurrencySceneViewModel) {
        self.model = model
    }

    public var body: some View {
        List(model.list) { section in
            Section(section.section) {
                ForEach(section.values) {
                    ListItemSelectionView(
                        title: $0.title,
                        titleExtra: .none,
                        titleTag: .none,
                        titleTagType: .none,
                        subtitle: .none,
                        subtitleExtra: .none,
                        value: $0.value.currency,
                        selection: model.currency,
                    ) {
                        onSelectCurrency($0)
                    }
                }
            }
            .listRowBackground(Colors.WowPay.surface)
        }
        .disabled(isChangingCurrency)
        .overlay {
            if isChangingCurrency {
                LoadingView()
            }
        }
        .listSectionSpacing(.compact)
        .scrollContentBackground(.hidden)
        .wowPayBackground()
        .navigationTitle(model.title)
        .alertSheet($isPresentingAlertMessage)
    }
}

// MARK: - Actions

extension CurrencyScene {
    private func onSelectCurrency(_ currency: Currency) {
        guard currency != model.currency, !isChangingCurrency else { return }
        isChangingCurrency = true

        Task {
            defer { isChangingCurrency = false }
            do {
                try await model.setCurrency(currency)
            } catch {
                isPresentingAlertMessage = AlertMessage(message: error.localizedDescription)
                return
            }
            dismiss()
        }
    }
}
