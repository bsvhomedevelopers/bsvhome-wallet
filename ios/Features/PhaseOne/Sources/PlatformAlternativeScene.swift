import Components
import Localization
import Style
import SwiftUI
import UniformTypeIdentifiers

public struct PlatformAlternativeScene: View {
    @Bindable private var model: PlatformAlternativeViewModel

    public init(model: PlatformAlternativeViewModel) {
        self.model = model
    }

    public var body: some View {
        PhaseOnePage(title: Localized.phaseoneAlternative, isBusy: model.isBusy, error: model.error) {
            Text(Localized.phaseoneIdentityHint).font(.body.weight(.medium))
            NavigationLink { enrollment } label: { Label(Localized.phaseoneAlternativeEnroll, systemImage: "key") }
            PublicKeyInput(model: model)
            ForEach(model.keys, id: \.id) { key in
                VStack(alignment: .leading, spacing: Spacing.small) {
                    Text(key.label).font(.callout.bold())
                    Text(key.fingerprint).font(.caption).textSelection(.enabled)
                    Text(status(key.status))
                    if key.status == "ACTIVE" {
                        Button(Localized.phaseoneAlternativeApply) { Task { await model.recovery(key.id) } }.buttonStyle(.wowPaySecondary())
                        Button(Localized.phaseoneRevoke, role: .destructive) { Task { await model.revoke(key.id) } }
                    }
                }.wowPayCard()
            }
            if model.keys.isEmpty {
                Text(Localized.phaseoneAlternativeNoKey).font(.caption)
            }
            requestDetails
            Button(Localized.phaseoneRefresh) { Task { await model.load() } }.buttonStyle(.wowPaySecondary())
        }
        .disabled(model.isBusy)
        .task { await model.load() }
        .refreshable { await model.load() }
        .navigationDestination(isPresented: Binding(get: { model.challenge != nil }, set: {
            if !$0 {
                model.cancel()
            }
        })) { challengePage }
    }

    private struct PublicKeyInput: View {
        @Bindable var model: PlatformAlternativeViewModel
        @State private var importing = false

        var body: some View {
            VStack(alignment: .leading, spacing: Spacing.small) {
                Text(Localized.phaseonePublicKey).font(.callout.bold())
                Text(Localized.phaseoneAlternativePublicHint).font(.caption)
                TextField(Localized.phaseonePublicKey, text: $model.publicKey, axis: .vertical)
                    .textInputAutocapitalization(.never).autocorrectionDisabled().wowPayInput()
                Button(Localized.phaseoneImportFile) { importing = true }
                    .buttonStyle(.wowPaySecondary())
                    .accessibilityIdentifier("phaseOneImportPublicKey")
                    .fileImporter(isPresented: $importing, allowedContentTypes: [.plainText]) { result in
                        switch result {
                        case let .success(url): Task { await model.importPublicKey(url) }
                        case .failure: model.fileFailed()
                        }
                    }
                Button(Localized.phaseoneAlternativeCheckKey) { Task { await model.checkPublicKey() } }.buttonStyle(.wowPaySecondary())
                if let fingerprint = model.fingerprint {
                    Text(fingerprint).font(.caption).textSelection(.enabled)
                }
            }.wowPayCard()
        }
    }

    private var enrollment: some View {
        PhaseOnePage(title: Localized.phaseoneAlternativeEnroll, isBusy: model.isBusy, error: model.error) {
            Text(Localized.phaseoneIdentityHint).font(.body.weight(.medium))
            PublicKeyInput(model: model)
            TextField(Localized.phaseoneLabel, text: $model.label).wowPayInput()
            Button(Localized.phaseoneAlternativeEnroll) { Task { await model.enrollment() } }.buttonStyle(.wowPay())
        }.disabled(model.isBusy)
    }

    private var challengePage: some View {
        PhaseOnePage(title: Localized.phaseoneChallenge, isBusy: model.isBusy, error: model.error) {
            if let challenge = model.challenge {
                VStack(alignment: .leading, spacing: Spacing.small) {
                    Text(purpose(challenge.purpose)).font(.callout.bold())
                    LabeledContent(Localized.phaseoneOfflineAccount, value: challenge.userId)
                    LabeledContent(Localized.phaseoneLabel, value: challenge.label)
                    Text(Localized.phaseoneFingerprint).font(.caption)
                    Text(challenge.fingerprint).textSelection(.enabled)
                    Text(Localized.phaseoneOfflineActionHash).font(.caption)
                    Text(challenge.actionHash).textSelection(.enabled)
                    Text(challenge.expiresAt).font(.caption)
                }.wowPayCard()
                Text(Localized.phaseoneOfflineChallengeWarning).font(.caption)
                ChallengeDocumentActions(model: model)
                if model.proofReady {
                    Text(Localized.phaseoneAlternativeProofVerified).font(.caption)
                }
                Button(purpose(challenge.purpose)) { Task { await model.submitProof() } }.buttonStyle(.wowPay()).disabled(!model.proofReady)
            }
        }.disabled(model.isBusy)
    }

    private struct ChallengeDocumentActions: View {
        let model: PlatformAlternativeViewModel
        @State private var importingProof = false
        @State private var exporting = false

        var body: some View {
            VStack(spacing: Spacing.wowPay.card) {
                Button(Localized.phaseoneAlternativeExportChallenge) {
                    Task {
                        await model.exportChallenge()
                        if model.exportDocument != nil {
                            exporting = true
                        }
                    }
                }
                .buttonStyle(.wowPaySecondary())
                .fileExporter(isPresented: $exporting, document: model.exportDocument, contentType: .json, defaultFilename: "wowpay-offline-identity-challenge") { result in
                    if case .failure = result {
                        model.fileFailed()
                    }
                }
                Button(Localized.phaseoneAlternativeImportProof) { importingProof = true }
                    .buttonStyle(.wowPaySecondary())
                    .fileImporter(isPresented: $importingProof, allowedContentTypes: [.json]) { result in
                        if case let .success(url) = result {
                            Task { await model.importProof(url) }
                        } else {
                            model.fileFailed()
                        }
                    }
            }
        }
    }

    private var requestDetails: some View {
        VStack(alignment: .leading, spacing: Spacing.small) {
            Text(Localized.phaseoneRecovery).font(.callout.bold())
            Text(Localized.phaseoneAlternativeWaitHint).font(.caption)
            TextField(Localized.phaseoneAlternativeRequestId, text: $model.requestId).textInputAutocapitalization(.never).autocorrectionDisabled().wowPayInput()
            Button(Localized.phaseoneRefresh) { Task { await model.refreshRequest() } }.buttonStyle(.wowPaySecondary())
            if let request = model.request {
                Text(status(request.status))
                Text(Localized.phaseoneAlternativeReviewers + ": " + String(request.approved) + "/" + String(request.required))
                Text(request.notified ? Localized.phaseoneAlternativeNoticeAccepted : Localized.phaseoneAlternativeNoticePending).font(.caption)
                LabeledContent(Localized.phaseoneNotBefore, value: request.readyAt ?? "—")
                if request.actions.contains(.cancel) {
                    Button(Localized.Common.cancel) { Task { await model.cancelRequest() } }.buttonStyle(.wowPaySecondary())
                }
                if request.actions.contains(.register) {
                    Button(Localized.phaseoneAlternativeRegister) { Task { await model.registration() } }.buttonStyle(.wowPay())
                }
            }
        }.wowPayCard()
    }

    private func purpose(_ code: String) -> String {
        switch code {
        case "Enroll": Localized.phaseoneOfflinePurposeEnroll
        case "Recover": Localized.phaseoneOfflinePurposeRecover
        case "Register": Localized.phaseoneOfflinePurposeRegister
        default: Localized.phaseoneStateUnknown
        }
    }

    private func status(_ code: String) -> String {
        switch code {
        case "ACTIVE": Localized.phaseoneStateActive
        case "COMPLETE": Localized.phaseoneStateComplete
        case "WAIT_REVIEW": Localized.phaseoneStateReview
        case "APPROVED": Localized.phaseoneStateWaiting
        case "CANCELLED": Localized.phaseoneStateCancelled
        case "REVOKED": Localized.phaseoneStateRevoked
        case "EXPIRED": Localized.phaseoneStateExpired
        case "REJECTED": Localized.phaseoneStateRejected
        default: Localized.phaseoneStateUnknown
        }
    }
}
