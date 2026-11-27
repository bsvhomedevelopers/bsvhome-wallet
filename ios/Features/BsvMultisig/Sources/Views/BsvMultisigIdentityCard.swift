import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigIdentityCard: View {
    @Environment(\.bsvMultisigIdentityContext) private var context
    @State private var isShowingPicker = false
    var entry: BsvMultisigIdentityEntry?
    var isBusy = false

    var body: some View {
        if let context {
            let selected = context.options.first { $0.walletId == context.selectedWalletId }
            VStack(spacing: Spacing.wowPay.card) {
                VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                    Text(Localized.multisigMyIdentity).font(.headline)
                    if let selected {
                        identity(selected)
                    } else {
                        Text(Localized.multisigNoLocalIdentity)
                            .font(.caption)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                    }
                    if entry != nil {
                        Text(Localized.multisigIdentitySourceHint)
                            .font(.caption)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                    }
                    if context.options.count > 1 || selected == nil && !context.options.isEmpty {
                        Button(entry == nil ? Localized.multisigSwitchIdentity : Localized.multisigUseExistingIdentity) {
                            isShowingPicker = true
                        }
                        .buttonStyle(.wowPaySecondary())
                        .disabled(isBusy || context.isSelecting)
                        .accessibilityIdentifier("bsvMultisigIdentityPicker")
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                if let entry, let onCreateDedicated = context.onCreateDedicated {
                    Button(Localized.multisigCreateDedicatedIdentity) { onCreateDedicated(entry) }
                        .buttonStyle(.wowPaySecondary())
                        .disabled(isBusy || context.isSelecting)
                        .accessibilityIdentifier("bsvMultisigCreateDedicatedIdentity")
                }
            }
            .sheet(isPresented: $isShowingPicker) {
                NavigationStack {
                    List(context.options) { option in
                        Button {
                            isShowingPicker = false
                            if option.walletId != context.selectedWalletId {
                                context.onSelect(option.walletId)
                            }
                        } label: {
                            HStack(spacing: Spacing.wowPay.card) {
                                identity(option)
                                Spacer(minLength: .zero)
                                if option.walletId == context.selectedWalletId {
                                    Image(systemName: SystemImage.checkmark)
                                        .foregroundStyle(Colors.WowPay.brand)
                                }
                            }
                        }
                        .disabled(isBusy || context.isSelecting)
                        .accessibilityIdentifier("bsvMultisigIdentity-\(option.walletId)")
                    }
                    .navigationTitle(Localized.multisigSwitchIdentity)
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) {
                            Button(Localized.Common.cancel) { isShowingPicker = false }
                        }
                    }
                }
                .presentationDetents([.medium, .large])
            }
        }
    }

    private func identity(_ option: BsvMultisigIdentityOption) -> some View {
        VStack(alignment: .leading, spacing: Spacing.space4) {
            Text(option.name).font(.subheadline.weight(.semibold))
                .foregroundStyle(Colors.WowPay.heading)
            if option.isDedicated {
                Text(Localized.multisigDedicatedIdentity).font(.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            Text(option.address).font(.caption).textSelection(.enabled)
                .foregroundStyle(Colors.WowPay.secondaryText)
        }
    }
}
