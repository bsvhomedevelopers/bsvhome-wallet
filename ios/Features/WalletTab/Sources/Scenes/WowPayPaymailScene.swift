import Components
import struct BsvHomeCore.BsvHomePaymailBinding
import Localization
import Style
import SwiftUI

public struct WowPayPaymailScene: View {
    @State private var model: WowPayPaymailViewModel

    public init(model: WowPayPaymailViewModel) {
        _model = State(initialValue: model)
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
                switch model.state {
                case .idle, .loading:
                    LoadingView()
                        .frame(maxWidth: .infinity)
                case let .failed(message):
                    Text(message).font(.wowPay.body)
                    Button(Localized.Common.refresh) { Task { await model.load() } }
                        .buttonStyle(.wowPaySecondary())
                case let .loaded(snapshot):
                    ForEach(snapshot.bindings, id: \.handle) { binding in
                        bindingCard(binding)
                    }
                    if !snapshot.registrationDomains.isEmpty {
                        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                            Picker(Localized.paymailDomain, selection: $model.domain) {
                                ForEach(snapshot.registrationDomains, id: \.self) { Text(verbatim: $0).tag($0) }
                            }
                            .disabled(model.operation != .idle)
                            WowPayPaymailForm(
                                title: Localized.wowpayPaymailSetup,
                                description: Localized.wowpayPaymailDescription,
                                fieldTitle: Localized.Wallet.name,
                                placeholder: Localized.Wallet.name,
                                saveTitle: Localized.wowpayPaymailSave,
                                doneTitle: Localized.Common.done,
                                name: $model.alias,
                                isSaving: model.isBusy,
                                isEnabled: model.operation == .idle && !model.alias.isEmpty,
                                onSave: model.confirmRegistration,
                            )
                        }
                    }
                    Text(snapshot.address)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                        .textSelection(.enabled)
                }
                if let notice = model.notice {
                    Text(notice).font(.wowPay.body)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                        .accessibilityIdentifier("paymailNotice")
                }
            }
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .navigationTitle(Localized.wowpayPaymail)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(Localized.Common.refresh) { Task { await model.load() } }
                    .disabled(model.isBusy)
            }
        }
        .sheet(item: $model.confirmation) { confirmation in
            NavigationStack {
                ScrollView {
                    VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                        Text(confirmation.handle).font(.wowPay.cardTitle)
                        Text(confirmation.snapshot.address).font(.wowPay.body).textSelection(.enabled)
                        Text(Localized.wowpayAssetNetwork + ": BSV").font(.wowPay.caption)
                        if !confirmation.active {
                            Text(Localized.paymailDisableNotice).font(.wowPay.body)
                        }
                        StateButton(text: Localized.Common.continue, type: .wowPayPrimary()) {
                            Task { await model.submit(confirmation) }
                        }
                        .accessibilityIdentifier("confirmPaymailBinding")
                    }
                    .padding(Spacing.wowPay.page)
                }
                .wowPayBackground()
                .navigationTitle(confirmation.title)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button(Localized.Common.cancel) { model.confirmation = nil }
                    }
                }
            }
            .presentationDetents([.medium, .large])
        }
        .task { await model.load() }
        .refreshable { await model.load() }
        .onDisappear { model.cancel() }
    }

    private func bindingCard(_ binding: BsvHomePaymailBinding) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            Text(binding.handle).font(.wowPay.cardTitle).textSelection(.enabled)
            Text(binding.active ? Localized.paymailActive : Localized.paymailInactive)
                .font(.wowPay.body)
            Text(binding.address).font(.wowPay.caption).textSelection(.enabled)
            switch binding.availability {
            case .available:
                StateButton(text: binding.active ? Localized.paymailDisable : Localized.paymailEnable, type: .wowPayPrimary(model.isBusy ? .loading() : model.operation == .idle ? .normal : .disabled)) {
                    model.confirmActivity(binding)
                }
            case .domainRemoved:
                Text(Localized.paymailUnavailable).font(.wowPay.caption)
            case .addressMismatch:
                Text(Localized.paymailIdentityMismatch).font(.wowPay.caption)
            }
        }
        .wowPayCard()
    }
}
