import Components
import Localization
import Style
import SwiftUI

public struct BsvMultisigListView: View {
    @Bindable private var model: BsvMultisigViewModel
    @Environment(\.bsvMultisigIdentityContext) private var identityContext
    @State private var showsPending = false
    private let walletId: String?
    private let context: BsvMultisigListContext?

    public init(model: BsvMultisigViewModel, walletId: String? = nil, context: BsvMultisigListContext? = nil) {
        self.model = model
        self.walletId = walletId
        self.context = context
        _showsPending = State(initialValue: walletId != nil)
    }

    private var wallets: [String] {
        if showsPending, let pending = context?.pendingWallets {
            return pending.filter { walletId == nil || $0.id == walletId }.map(\.id)
        }
        return (context?.wallets ?? model.wallets).filter { (walletId == nil || $0.id == walletId) && (!showsPending || $0.personalConfigurationPending) }.map(\.id)
    }

    var pendingProposals: [String] {
        if let context {
            return context.proposals.filter { walletId == nil || $0.walletId == walletId }.map(\.id)
        }
        return model.proposals.filter { $0.isPending && (walletId == nil || $0.walletId == walletId) }.map(\.id)
    }

    private var pendingCount: Int {
        guard let walletId else { return Int(clamping: context?.pendingCount ?? UInt64(model.pendingCount)) }
        return (context?.wallets ?? model.wallets).filter { $0.id == walletId && $0.personalConfigurationPending }.count + pendingProposals.count
    }

    public var body: some View {
        content
            .bsvMultisigRefresh(automatically: context == nil) {
                if let refresh = context?.onRefresh {
                    await refresh()
                } else {
                    await model.refresh()
                }
            }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            BsvMultisigNotice(text: Localized.multisigPolicyNotice)
            if let error = context?.errorMessage ?? (context == nil ? model.errorMessage : nil) {
                BsvMultisigNotice(text: error, tone: .error)
                Button(Localized.Common.tryAgain) {
                    Task {
                        if let refresh = context?.onRefresh {
                            await refresh()
                        } else {
                            await model.refresh()
                        }
                    }
                }.buttonStyle(.wowPaySecondary())
            }
            HStack(spacing: Spacing.small) {
                filterTab(Localized.multisigMyWallets, isSelected: !showsPending) { showsPending = false }
                filterTab(Localized.multisigPending, count: pendingCount, isSelected: showsPending) { showsPending = true }
                Spacer(minLength: .zero)
            }
            if context?.isLoading ?? model.isLoading, wallets.isEmpty, !showsPending || pendingProposals.isEmpty {
                LoadingView().frame(maxWidth: .infinity)
            } else if (context?.errorMessage ?? (context == nil ? model.errorMessage : nil)) == nil, wallets.isEmpty, !showsPending || pendingProposals.isEmpty {
                Text(showsPending ? Localized.multisigEmptyPending : Localized.multisigEmptyWallets)
                    .font(.footnote).foregroundStyle(Colors.WowPay.secondaryText)
                    .frame(maxWidth: .infinity)
                    .padding(Spacing.wowPay.section)
            }
            ForEach(wallets, id: \.self) { id in
                if let wallet = (showsPending ? context?.pendingWallets ?? context?.wallets ?? model.wallets : context?.wallets ?? model.wallets).first(where: { $0.id == id }) {
                    BsvMultisigWalletCard(model: model, wallet: wallet, onSelect: context.map { context in { context.onOpenWallet(wallet) } })
                }
            }
            if showsPending {
                ForEach(pendingProposals, id: \.self) { id in
                    if let proposal = (context?.proposals ?? model.proposals).first(where: { $0.id == id }) {
                        BsvMultisigProposalCard(model: model, proposal: proposal, walletName: context.map { $0.wallets.first { $0.id == proposal.walletId }?.name ?? "" }, onSelect: context.map { context in { context.onOpenProposal(proposal) } })
                    }
                }
            }
            if walletId == nil {
                HStack(spacing: Spacing.wowPay.grid) {
                    NavigationLink(Localized.multisigCreate, value: BsvMultisigRoute.create).buttonStyle(.wowPay())
                    NavigationLink(Localized.multisigJoin, value: BsvMultisigRoute.join).buttonStyle(.wowPaySecondary())
                }
                .disabled(identityContext.map { context in
                    context.isSelecting || !context.options.contains { $0.walletId == context.selectedWalletId }
                } ?? false)
            }
        }
        .foregroundStyle(Colors.WowPay.heading)
    }

    private func filterTab(_ title: String, count: Int = 0, isSelected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: Spacing.space4) {
                Text(title).font(.wowPay.multisigTab)
                if count > 0 {
                    WowPayCountBadge(count: count)
                }
            }
            .foregroundStyle(isSelected ? Colors.WowPay.onBrand : Colors.WowPay.secondaryText)
            .padding(.horizontal, Spacing.space16)
            .frame(minHeight: Sizing.wowPay.multisigTabControlHeight)
            .background(isSelected ? Colors.WowPay.brand : Colors.WowPay.secondaryButton, in: RoundedRectangle(cornerRadius: Sizing.wowPay.paymailActionCorner))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}
