import Components
import Localization
import Style
import SwiftUI

struct BsvMultisigCreateScene: View {
    @Environment(BsvMultisigNavigation.self) private var navigation
    @Environment(\.bsvMultisigIdentityContext) private var identityContext
    @Bindable var model: BsvMultisigViewModel
    @State private var name = ""
    @State private var threshold: UInt32 = 2
    @State private var memberNames = [Localized.multisigMe, Localized.multisigMemberName(2), Localized.multisigMemberName(3)]
    @State private var beginTask: Task<Void, Never>?
    @State private var beginRevision: UInt = 0

    private var selectedIdentity: BsvMultisigIdentityOption? {
        identityContext?.options.first { $0.walletId == identityContext?.selectedWalletId }
    }

    private var canReview: Bool {
        let hasIdentity = identityContext.map { context in
            !context.isSelecting && context.options.contains { $0.walletId == context.selectedWalletId }
        } ?? true
        return hasIdentity && !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && memberNames.allSatisfy { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    }

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigCreate, subtitle: Localized.multisigCreateSubtitle) {
            BsvMultisigIdentityCard(entry: .create, isBusy: model.isWorking || model.isLoading)
            HStack(spacing: Spacing.small) {
                Text(Localized.multisigStepRules).foregroundStyle(Colors.WowPay.brand).fontWeight(.bold)
                Spacer(minLength: .zero)
                Text(Localized.multisigStepInvite)
                Spacer(minLength: .zero)
                Text(Localized.multisigStepConfirm)
            }
            .font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
            VStack(alignment: .leading, spacing: Spacing.wowPay.multisigField) {
                Text(Localized.Wallet.name).font(.wowPay.multisigLabel)
                TextField(Localized.multisigNamePlaceholder, text: $name)
                    .font(.body).wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                    .accessibilityIdentifier("bsvMultisigCreateName")
                    .onChange(of: name) { _, value in name = String(value.prefix(30)) }
                BsvMultisigValueRow(title: Localized.wowpayAssetNetwork, value: Localized.multisigMainnet + " · BSV")
                Text(Localized.multisigThresholdQuestion).font(.wowPay.multisigLabel)
                Menu {
                    ForEach(2 ... memberNames.count, id: \.self) { number in
                        Button(Localized.multisigThresholdOption(number, memberNames.count)) { threshold = UInt32(number) }
                            .accessibilityIdentifier("bsvMultisigCreateThresholdOption_\(number)")
                    }
                } label: {
                    HStack {
                        Text(Localized.multisigThresholdOption(Int(threshold), memberNames.count))
                        Spacer(minLength: .zero)
                        Image(systemName: "chevron.down")
                    }
                    .font(.body).wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                }
                .accessibilityIdentifier("bsvMultisigCreateThreshold")
                Text(Localized.multisigWeightNotice(Int(threshold))).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
            }
            .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
            VStack(alignment: .leading, spacing: Spacing.small) {
                HStack {
                    Text(Localized.multisigMembers).font(.headline)
                    Spacer()
                    Button(Localized.multisigAddMember) { memberNames.append(Localized.multisigMemberName(memberNames.count + 1)) }
                        .font(.caption).foregroundStyle(Colors.WowPay.brand).disabled(memberNames.count == 5)
                        .accessibilityIdentifier("bsvMultisigCreateAddMember")
                }
                ForEach(memberNames.indices, id: \.self) { index in
                    HStack(spacing: Spacing.wowPay.grid) {
                        Text(String(index + 1)).font(.headline).foregroundStyle(Colors.WowPay.brand)
                            .frame(width: Sizing.wowPay.multisigAvatar, height: Sizing.wowPay.multisigAvatar)
                            .background(Colors.WowPay.serviceSurface, in: Circle())
                        VStack(alignment: .leading, spacing: Spacing.space4) {
                            TextField(Localized.multisigMemberName(index + 1), text: $memberNames[index])
                                .font(.body).wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                                .accessibilityIdentifier("bsvMultisigCreateMember_\(index + 1)")
                                .onChange(of: memberNames[index]) { _, value in memberNames[index] = String(value.prefix(20)) }
                            Text(index == 0 ? Localized.multisigLocalMemberNotice : Localized.multisigInvitedMemberNotice)
                                .font(.caption2).foregroundStyle(Colors.WowPay.secondaryText)
                        }
                        if index > 1 {
                            Button(Localized.multisigRemoveMember) {
                                memberNames.remove(at: index)
                                threshold = min(threshold, UInt32(memberNames.count))
                            }
                            .font(.caption).foregroundStyle(Colors.WowPay.brand)
                        }
                    }
                    .padding(.vertical, Spacing.wowPay.multisigMember)
                }
            }
            .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
            BsvMultisigNotice(text: Localized.multisigCreateNotice, tone: .warning)
            StateButton(text: Localized.multisigConfirmRules, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : canReview ? .normal : .disabled), action: beginReview)
                .disabled(model.isLoading)
                .accessibilityIdentifier("bsvMultisigCreateReview")
        }
        .onDisappear(perform: cancelBegin)
        .onChange(of: ObjectIdentifier(model)) { _, _ in cancelBegin() }
        .onChange(of: selectedIdentity) { _, _ in
            cancelBegin()
            name = ""
            threshold = 2
            memberNames = [Localized.multisigMe, Localized.multisigMemberName(2), Localized.multisigMemberName(3)]
        }
    }

    private func beginReview() {
        guard beginTask == nil else { return }
        let reviewedName = name
        let reviewedThreshold = threshold
        let reviewedMembers = memberNames
        let reviewedSigner = identityContext?.selectedWalletId
        beginRevision &+= 1
        let currentRevision = beginRevision
        beginTask = Task {
            defer {
                if currentRevision == beginRevision {
                    beginTask = nil
                }
            }
            if let review = await model.beginCreation(name: reviewedName, threshold: reviewedThreshold, memberNames: reviewedMembers, signerWalletId: reviewedSigner),
               !Task.isCancelled, currentRevision == beginRevision, name == reviewedName, threshold == reviewedThreshold, memberNames == reviewedMembers
            {
                navigation.append(.createReview(review))
            }
        }
    }

    private func cancelBegin() {
        beginRevision &+= 1
        beginTask?.cancel()
        beginTask = nil
    }
}
