import Components
import Foundation
import enum BsvHomeCore.BsvHomeBsvMultisigError
import struct BsvHomeCore.BsvHomeBsvMultisigFundingPayer
import Localization
import Style
import SwiftUI

public struct BsvMultisigPublicFundingScene: View {
    private let sourceContextId: UUID
    private let entry: BsvMultisigPublicFundingEntry
    private let eligiblePayers: StateViewType<[BsvHomeBsvMultisigFundingPayer]>
    private let model: BsvMultisigPublicFundingViewModel?
    private let onSelectPayer: @MainActor (BsvHomeBsvMultisigFundingPayer) -> Void
    private let onRetryPayers: @MainActor () -> Void
    private let onOpenRecovery: @MainActor () -> Void
    private let onClose: @MainActor () -> Void

    public init(sourceContextId: UUID, entry: BsvMultisigPublicFundingEntry, eligiblePayers: StateViewType<[BsvHomeBsvMultisigFundingPayer]>, model: BsvMultisigPublicFundingViewModel?, onSelectPayer: @escaping @MainActor (BsvHomeBsvMultisigFundingPayer) -> Void, onRetryPayers: @escaping @MainActor () -> Void, onOpenRecovery: @escaping @MainActor () -> Void, onClose: @escaping @MainActor () -> Void) {
        self.sourceContextId = sourceContextId
        self.entry = entry
        self.eligiblePayers = eligiblePayers
        self.model = model
        self.onSelectPayer = onSelectPayer
        self.onRetryPayers = onRetryPayers
        self.onOpenRecovery = onOpenRecovery
        self.onClose = onClose
    }

    public var body: some View {
        Form(entry: entry, eligiblePayers: eligiblePayers, model: model, onSelectPayer: onSelectPayer, onRetryPayers: onRetryPayers, onOpenRecovery: onOpenRecovery, onClose: onClose)
            .id(sourceContextId)
    }

    private struct Form: View {
        @Environment(NavigationPathState.self) private var navigation: NavigationPathState?
        @Environment(\.scenePhase) private var phase
        @State private var amount: String
        @State private var entryDepth: Int?
        let entry: BsvMultisigPublicFundingEntry
        let eligiblePayers: StateViewType<[BsvHomeBsvMultisigFundingPayer]>
        let model: BsvMultisigPublicFundingViewModel?
        let onSelectPayer: @MainActor (BsvHomeBsvMultisigFundingPayer) -> Void
        let onRetryPayers: @MainActor () -> Void
        let onOpenRecovery: @MainActor () -> Void
        let onClose: @MainActor () -> Void

        init(entry: BsvMultisigPublicFundingEntry, eligiblePayers: StateViewType<[BsvHomeBsvMultisigFundingPayer]>, model: BsvMultisigPublicFundingViewModel?, onSelectPayer: @escaping @MainActor (BsvHomeBsvMultisigFundingPayer) -> Void, onRetryPayers: @escaping @MainActor () -> Void, onOpenRecovery: @escaping @MainActor () -> Void, onClose: @escaping @MainActor () -> Void) {
            self.entry = entry
            self.eligiblePayers = eligiblePayers
            self.model = model
            self.onSelectPayer = onSelectPayer
            self.onRetryPayers = onRetryPayers
            self.onOpenRecovery = onOpenRecovery
            self.onClose = onClose
            _amount = State(initialValue: BsvMultisigAmountFormatter().input(entry.request?.amount ?? ""))
        }

        var body: some View {
            Group {
                if let model {
                    BsvMultisigPublicFundingPage(model: model, title: Localized.multisigFunding) { content }
                } else {
                    BsvMultisigPageLayout(title: Localized.multisigFunding) { content }
                }
            }
            .navigationDestination(for: BsvMultisigPublicFundingRoute.self) { route in
                if let model {
                    switch route {
                    case let .review(id): BsvMultisigPublicFundingReviewScene(model: model, preparationId: id)
                    case let .result(id): BsvMultisigPublicFundingResultScene(model: model, operationId: id, onClose: close)
                    case .recovery: BsvMultisigPublicFundingRecoveryScene(model: model)
                    }
                } else {
                    BsvMultisigNotice(text: Localized.multisigFundingNoPayer, tone: .warning)
                }
            }
            .onAppear {
                if entryDepth == nil {
                    entryDepth = navigation?.binding.wrappedValue.count
                }
                model?.startLoading()
            }
            .onChange(of: model?.id) { _, _ in
                popToForm()
                model?.startLoading()
            }
            .onChange(of: phase) { _, value in
                if value == .background {
                    model?.cancelActions()
                } else if value == .active {
                    model?.startLoading()
                }
            }
        }

        @ViewBuilder private var content: some View {
            if eligiblePayers.isLoading || eligiblePayers.isNoData {
                LoadingView()
            } else if case let .error(error) = eligiblePayers {
                BsvMultisigNotice(text: BsvHomeBsvMultisigError.displayedMessage(for: error) ?? Localized.Errors.errorOccurred, tone: .warning)
                Button(Localized.Common.tryAgain, action: onRetryPayers).buttonStyle(.wowPaySecondary())
            } else if let payers = eligiblePayers.value, payers.isEmpty {
                BsvMultisigNotice(text: Localized.multisigFundingNoPayer, tone: .warning)
            } else if navigation == nil {
                BsvMultisigNotice(text: Localized.Errors.notSupported, tone: .warning)
            } else if let payers = eligiblePayers.value {
                VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                    BsvMultisigFundingPayerPicker(payers: payers, selectedWalletId: model?.payer.id, onSelect: selectPayer)
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                if let request = entry.request {
                    VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                        BsvMultisigValueRow(title: Localized.multisigFundingTarget, value: request.name)
                        BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet)
                        Text(Localized.multisigAmount).font(.wowPay.multisigLabel)
                        TextField("0.00000000", text: Binding(get: { amount }, set: { value in
                            if value != amount {
                                model?.cancelActions(); amount = value
                            }
                        }))
                        .keyboardType(.decimalPad)
                        .wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                        .accessibilityIdentifier("bsvPublicFundingAmount")
                    }
                    .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                    if let model {
                        StateButton(text: Localized.multisigFundingReview, type: .wowPayPrimary(model.isWorking ? .loading() : model.canPrepare(amount: amount) ? .normal : .disabled)) {
                            model.prepare(amount: amount, navigation: navigation) { push(.review($0.id)) }
                        }
                        .accessibilityIdentifier("bsvPublicFundingPrepare")
                    }
                }
                if let model {
                    if model.recoveryState.isLoading {
                        LoadingView()
                    }
                    if model.recoveryState.isError {
                        Button(Localized.Common.tryAgain, action: model.reloadResults).buttonStyle(.wowPaySecondary())
                    }
                    Button(Localized.multisigFundingResult) { push(.recovery) }.buttonStyle(.wowPaySecondary())
                        .accessibilityIdentifier("bsvPublicFundingRecovery")
                }
            }
            if case .payment = entry {
                Button(Localized.multisigFundingResult) { model?.cancel(); onOpenRecovery() }.buttonStyle(.wowPaySecondary())
                    .accessibilityIdentifier("bsvPublicFundingOpenRecovery")
            }
            Button(Localized.Common.cancel, action: close).buttonStyle(.wowPaySecondary())
        }

        private func push(_ route: BsvMultisigPublicFundingRoute) {
            navigation?.binding.wrappedValue.append(route)
        }

        private func selectPayer(_ payer: BsvHomeBsvMultisigFundingPayer) {
            guard payer.walletId != model?.payer.id else { return }
            model?.cancel()
            popToForm()
            onSelectPayer(payer)
        }

        private func popToForm() {
            if let navigation, let entryDepth {
                let count = navigation.binding.wrappedValue.count
                if count > entryDepth {
                    navigation.removeLast(count - entryDepth)
                }
            }
        }

        private func close() {
            model?.cancel()
            onClose()
        }
    }
}
