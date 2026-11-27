import Components
import Localization
import Style
import SwiftUI
import UIKit

struct BsvMultisigJoinScene: View {
    @Environment(BsvMultisigNavigation.self) private var navigation
    @Environment(\.bsvMultisigIdentityContext) private var identityContext
    @Environment(\.bsvMultisigInvitationScanner) private var onScanInvitation
    @Bindable var model: BsvMultisigViewModel
    @State private var token: String
    @State private var initialPreviewPending: Bool
    @State private var previewTask: Task<Void, Never>?
    @State private var previewRevision: UInt = 0

    init(model: BsvMultisigViewModel, initialInvitation: String? = nil) {
        self.model = model
        _token = State(initialValue: initialInvitation ?? "")
        _initialPreviewPending = State(initialValue: initialInvitation != nil)
    }

    var body: some View {
        BsvMultisigPage(model: model, title: Localized.multisigJoin, subtitle: Localized.multisigJoinSubtitle) {
            BsvMultisigIdentityCard(entry: .join, isBusy: model.isWorking || model.isLoading)
            VStack(alignment: .leading, spacing: Spacing.space12) {
                Text(Localized.multisigInvitation).font(.wowPay.multisigLabel)
                TextField(Localized.multisigInvitationPlaceholder, text: $token, axis: .vertical)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                    .font(.body).wowPayInput(surface: Colors.WowPay.multisigInputSurface)
                    .disabled(model.isWorking)
                    .accessibilityIdentifier("bsvMultisigInvitationInput")
                HStack(spacing: Spacing.wowPay.grid) {
                    Button(Localized.Common.paste) { token = UIPasteboard.general.string ?? "" }
                        .buttonStyle(.wowPaySecondary())
                        .disabled(model.isWorking)
                    if let onScanInvitation {
                        Button(Localized.multisigScanInvitation) {
                            onScanInvitation { value in
                                token = value
                                previewInvitation()
                            }
                        }
                        .buttonStyle(.wowPaySecondary())
                        .disabled(model.isWorking || identityContext?.isSelecting == true)
                        .accessibilityIdentifier("bsvMultisigScanInvitation")
                    }
                }
                Text(Localized.multisigJoinNotice).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
            }
            .wowPayCard(cornerRadius: Sizing.wowPay.compactCardCorner)
            StateButton(text: Localized.multisigViewInvitation, textStyle: .init(font: .headline, color: Colors.WowPay.onBrand), type: .wowPayPrimary(model.isWorking ? .loading() : token.isEmpty ? .disabled : .normal)) {
                previewInvitation()
            }
            .disabled(model.isLoading || identityContext?.isSelecting == true)
            .accessibilityIdentifier("bsvMultisigPreviewInvitation")
        }
        .onAppear(perform: previewInitialInvitation)
        .onDisappear(perform: cancelPreview)
        .onChange(of: ObjectIdentifier(model)) { _, _ in cancelPreview(); previewInitialInvitation() }
        .onChange(of: identityContext?.selectedWalletId) { _, _ in cancelPreview(); previewInitialInvitation() }
        .onChange(of: identityContext?.isSelecting) { _, _ in previewInitialInvitation() }
        .onChange(of: model.isLoading) { _, _ in previewInitialInvitation() }
        .onChange(of: model.isWorking) { _, _ in previewInitialInvitation() }
    }

    private func previewInitialInvitation() {
        guard initialPreviewPending, identityContext?.selectedWalletId != nil, identityContext?.isSelecting != true, !model.isLoading, !model.isWorking, !model.isCancelled else { return }
        previewInvitation()
    }

    private func previewInvitation() {
        guard previewTask == nil else { return }
        let value = token
        let sourceModel = model
        let identity = identityContext?.selectedWalletId
        previewRevision &+= 1
        let currentRevision = previewRevision
        previewTask = Task {
            defer {
                if currentRevision == previewRevision {
                    previewTask = nil
                }
            }
            guard !Task.isCancelled, !sourceModel.isCancelled, !sourceModel.isLoading, !sourceModel.isWorking,
                  currentRevision == previewRevision, identity == identityContext?.selectedWalletId else { return }
            initialPreviewPending = false
            if let preview = await sourceModel.previewInvitation(value), !Task.isCancelled, !sourceModel.isCancelled, ObjectIdentifier(model) == ObjectIdentifier(sourceModel), identity == identityContext?.selectedWalletId, currentRevision == previewRevision, value == token {
                navigation.append(.joinReview(preview))
            }
        }
    }

    private func cancelPreview() {
        previewRevision &+= 1
        previewTask?.cancel()
        previewTask = nil
    }
}
