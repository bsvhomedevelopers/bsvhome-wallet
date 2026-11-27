import Components
import struct BsvHomeCore.BsvHomeBsvMultisigPaymailSettings
import Localization
import Style
import SwiftUI

struct BsvMultisigPaymailScene: View {
    private struct Mutation: Identifiable {
        let id = UUID()
        let settings: BsvHomeBsvMultisigPaymailSettings
        let alias: String
        let active: Bool
        let renaming: Bool

        var title: String {
            renaming ? Localized.Common.save : active ? Localized.paymailEnable : Localized.paymailDisable
        }

        var message: String {
            renaming ? Localized.multisigPaymailRenameConfirm(alias + "@" + settings.domain)
                : active ? Localized.multisigPaymailEnableConfirm : Localized.multisigPaymailDisableConfirm
        }
    }

    @Environment(NavigationPathState.self) private var navigationOwner: NavigationPathState?
    @Environment(\.scenePhase) private var scenePhase
    @Bindable var model: BsvMultisigViewModel
    let walletId: String
    @State private var alias = ""
    @State private var mutation: Mutation?

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.wowpayPaymailManage, subtitle: model.wallet(walletId)?.name ?? "") {
            if let settings = model.paymailSettings[walletId] {
                VStack(alignment: .leading, spacing: Spacing.space12) {
                    Text(Localized.multisigPaymailAddress).font(.headline)
                    Text(settings.handle).textSelection(.enabled).foregroundStyle(Colors.WowPay.brand)
                    if settings.active {
                        HStack(spacing: Spacing.space8) {
                            Button(Localized.Common.copy) { model.copy(settings.handle) }
                            Button(Localized.Common.share) { model.share(settings.handle) }
                        }
                    }
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                VStack(alignment: .leading, spacing: Spacing.space12) {
                    Text(Localized.Transaction.status).font(.headline)
                    Text(settings.active ? Localized.paymailActive : Localized.paymailInactive)
                        .foregroundStyle(settings.active ? Colors.WowPay.brand : Colors.WowPay.secondaryText)
                }
                .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                if settings.canManage {
                    VStack(alignment: .leading, spacing: Spacing.space12) {
                        TextField(Localized.wowpayPaymailName, text: $alias)
                            .textInputAutocapitalization(.never).autocorrectionDisabled()
                            .wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                            .accessibilityIdentifier("bsvMultisigPaymailAlias")
                        Text("@" + settings.domain).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                        Button(Localized.Common.save) {
                            if let canonicalAlias = model.canonicalPaymailAlias(alias) {
                                mutation = Mutation(settings: settings, alias: canonicalAlias, active: settings.active, renaming: true)
                            }
                        }
                        .buttonStyle(.wowPay())
                        .disabled(alias.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || alias == settings.alias)
                    }
                    .disabled(model.isWorking || model.paymailLoading.contains(walletId))
                    .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
                    Button(settings.active ? Localized.paymailDisable : Localized.paymailEnable) {
                        mutation = Mutation(settings: settings, alias: settings.alias, active: !settings.active, renaming: false)
                    }
                    .buttonStyle(.wowPaySecondary())
                    .disabled(model.isWorking || model.paymailLoading.contains(walletId))
                } else if settings.version > 0 || settings.active {
                    Text(Localized.multisigPaymailCreatorOnly).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                }
            } else if model.isWorking || model.paymailLoading.contains(walletId) {
                LoadingView()
            }
            if let error = model.paymailErrors[walletId] {
                BsvMultisigNotice(text: error, tone: .error)
            }
            Button(Localized.Common.refresh) { Task { await model.loadPaymail(walletId) } }.buttonStyle(.wowPaySecondary())
                .disabled(model.isWorking || model.paymailLoading.contains(walletId))
        }
        .accessibilityIdentifier("bsvMultisigPaymailSettings")
        .task { await model.loadPaymail(walletId) }
        .onChange(of: model.paymailSettings[walletId], initial: true) { _, settings in
            if let settings {
                alias = settings.alias
            }
            mutation = nil
        }
        .alert(mutation?.title ?? Localized.Common.save, isPresented: mutationBinding, presenting: mutation) { pending in
            Button(Localized.Common.done) {
                mutation = nil
                model.updatePaymail(pending.settings, alias: pending.alias, active: pending.active, navigation: navigationOwner)
            }
            Button(Localized.Common.cancel, role: .cancel) { mutation = nil }
        } message: { Text($0.message) }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                cancelMutation()
            }
        }
    }

    private var mutationBinding: Binding<Bool> {
        let presentedId = mutation?.id
        return Binding(get: { mutation != nil }, set: {
            if !$0, mutation?.id == presentedId {
                mutation = nil
            }
        })
    }

    private func cancelMutation() {
        model.cancelActions()
        mutation = nil
    }
}
