import Components
import Localization
import Style
import SwiftUI

public struct BsvMultisigWalletScene<Header: View>: View {
    public enum Presentation: Equatable, Sendable {
        case navigation
        case tab
    }

    @Environment(\.dismiss) private var dismiss
    @Bindable private var model: BsvMultisigViewModel
    private let walletId: String
    private let presentation: Presentation
    private let header: Header

    public init(model: BsvMultisigViewModel, walletId: String, presentation: Presentation = .navigation, @ViewBuilder header: () -> Header) {
        self.model = model
        self.walletId = walletId
        self.presentation = presentation
        self.header = header()
    }

    public var body: some View {
        BsvMultisigPage(model: model, title: model.wallet(walletId)?.name ?? Localized.multisigWalletTitle, subtitle: subtitle, navigationBarVisibility: navigationBarVisibility) {
            header
            if let wallet = model.wallet(walletId) {
                VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                    Text(wallet.name).font(.wowPay.label)
                    Text(wallet.status == .active ? model.amount(wallet.balanceSatoshis, symbol: true) : "— BSV")
                        .font(.wowPay.multisigTotal)
                        .padding(.vertical, Spacing.small)
                    Text(Localized.multisigTransferRuleNotice(Int(wallet.threshold)))
                        .font(.caption).foregroundStyle(Colors.WowPay.onBrandSecondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .foregroundStyle(Colors.WowPay.onBrand)
                .padding(Spacing.wowPay.balancePadding)
                .background(WowPayGradients.balance, in: RoundedRectangle(cornerRadius: Sizing.wowPay.cardCorner))
                VStack(alignment: .leading, spacing: Spacing.space12) {
                    HStack {
                        Text(Localized.multisigMembers + " · " + wallet.rule).font(.headline)
                        Spacer(minLength: .zero)
                        NavigationLink(Localized.multisigMembersRules, value: BsvMultisigRoute.members(walletId)).font(.wowPay.caption)
                    }
                    Text(wallet.members.map(\.name).joined(separator: " · "))
                        .font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                if !wallet.canTransact {
                    BsvMultisigNotice(text: wallet.configurationStatusTitle, tone: .warning)
                    if wallet.personalConfigurationPending {
                        NavigationLink(Localized.multisigLocalReview, value: BsvMultisigRoute.configuration(walletId)).buttonStyle(.wowPaySecondary())
                    }
                }
                BsvMultisigIdentityCard(isBusy: model.isWorking || model.isLoading)
                if wallet.balanceIsStale {
                    BsvMultisigNotice(text: Localized.multisigErrorProvider, tone: .warning)
                }
                HStack(spacing: Spacing.wowPay.grid) {
                    NavigationLink(Localized.multisigSend, value: BsvMultisigRoute.send(walletId)).buttonStyle(.wowPay())
                    NavigationLink(Localized.Wallet.receive, value: BsvMultisigRoute.receive(walletId)).buttonStyle(.wowPaySecondary())
                }
                .disabled(!wallet.canTransact || model.isWorking || model.isLoading)
                HStack {
                    Text(Localized.multisigPending).font(.headline)
                    Spacer()
                    NavigationLink(Localized.wowpayViewAll, value: BsvMultisigRoute.pending(walletId))
                        .font(.caption).foregroundStyle(Colors.WowPay.brand)
                }
                .padding(.top, Spacing.space12)
                if model.proposals(walletId: walletId).filter(\.isPending).isEmpty {
                    Text(Localized.multisigEmptyPending).font(.footnote).foregroundStyle(Colors.WowPay.secondaryText)
                        .frame(maxWidth: .infinity).padding(Spacing.wowPay.section)
                        .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                }
                ForEach(Array(model.proposals(walletId: walletId).filter(\.isPending).prefix(3)), id: \.id) { BsvMultisigProposalCard(model: model, proposal: $0) }
                HStack(spacing: Spacing.wowPay.grid) {
                    Text(Localized.wowpayRecentTransactions).font(.headline)
                    Spacer()
                    NavigationLink(Localized.wowpayViewAll, value: BsvMultisigRoute.history(walletId)).font(.wowPay.caption)
                }
                if let error = model.activityErrors[walletId] {
                    BsvMultisigNotice(text: error, tone: .error)
                    Button(Localized.Common.tryAgain) { Task { await model.refreshActivity(walletId) } }.buttonStyle(.wowPaySecondary())
                }
                if let activity = model.activities[walletId] {
                    ForEach(Array(activity.items.prefix(3)), id: \.transactionId) { BsvMultisigActivityCard(model: model, walletId: walletId, item: $0) }
                } else if wallet.status == .active, model.activityErrors[walletId] == nil {
                    LoadingView()
                }
                NavigationLink(Localized.multisigWalletSettings, value: BsvMultisigRoute.settings(walletId)).buttonStyle(.wowPaySecondary())
            } else if case .tab = presentation {
                if model.isLoading || model.isWorking {
                    LoadingView().frame(maxWidth: .infinity)
                        .accessibilityIdentifier("collectiveWalletLoading")
                } else {
                    BsvMultisigNotice(text: model.errorMessage ?? Localized.multisigErrorMissingRecord, tone: .error)
                        .accessibilityIdentifier("collectiveWalletUnavailable")
                    Button(Localized.Common.tryAgain) { Task { await model.refresh() } }
                        .buttonStyle(.wowPaySecondary())
                        .accessibilityIdentifier("collectiveWalletRetry")
                }
            }
        }
        .bsvMultisigRefresh(automatically: presentation == .navigation) {
            if case .tab = presentation, model.wallet(walletId) == nil {
                await model.refresh()
            } else {
                await model.refreshWalletActivity(walletId)
            }
        }
        .task(id: model.wallet(walletId)?.status) {
            if model.wallet(walletId)?.status == .active {
                await model.refreshActivity(walletId)
            }
        }
        .onChange(of: model.wallet(walletId)) { _, wallet in
            if case .navigation = presentation, wallet == nil {
                dismiss()
            }
        }
    }

    private var navigationBarVisibility: Visibility {
        switch presentation {
        case .navigation: .visible
        case .tab: .hidden
        }
    }

    private var subtitle: String {
        switch presentation {
        case .navigation: Localized.multisigWalletSubtitle
        case .tab: ""
        }
    }
}

public extension BsvMultisigWalletScene where Header == EmptyView {
    init(model: BsvMultisigViewModel, walletId: String, presentation: Presentation = .navigation) {
        self.init(model: model, walletId: walletId, presentation: presentation) { EmptyView() }
    }
}
