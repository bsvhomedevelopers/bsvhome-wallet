import Components
import Foundation
import enum BsvHomeCore.BsvHomeOfflineIdentityPurpose
import Localization
import Style
import SwiftUI
import UniformTypeIdentifiers

public struct OfflineIdentityScene: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @State private var lifetime: OfflineIdentityLifetime?
    private let model: OfflineIdentityViewModel

    public init(model: OfflineIdentityViewModel) {
        self.model = model
    }

    public var body: some View {
        @Bindable var model = model
        NavigationStack(path: $model.path) {
            page(nil)
                .toolbar { ToolbarItem(placement: .topBarLeading) { Button(Localized.Common.cancel) { dismiss() } } }
                .navigationDestination(for: OfflineIdentityRoute.self) { page($0) }
        }
        .fileImporter(isPresented: $model.isImporting, allowedContentTypes: [.json, .plainText]) { model.imported($0) }
        .fileExporter(isPresented: $model.isExporting, document: model.document, contentType: exportContentType, defaultFilename: model.pendingExport?.filename ?? "wowpay-offline-identity.json") { result in
            Task { await model.exported(result) }
        }
        .onChange(of: scenePhase, initial: true) { _, phase in
            model.sceneChanged(phase)
            if phase == .active, model.publicState == nil, model.path.isEmpty {
                Task { await model.load() }
            }
        }
        .onChange(of: model.path, model.navigationChanged)
        .onReceive(NotificationCenter.default.publisher(for: UIScreen.capturedDidChangeNotification)) { _ in updateCapture() }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.protectedDataWillBecomeUnavailableNotification)) { _ in model.clearSensitiveState() }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.userDidTakeScreenshotNotification)) { _ in model.clearSensitiveState() }
        .onAppear {
            if lifetime == nil {
                lifetime = OfflineIdentityLifetime(model: model)
            }
            updateCapture()
        }
        .overlay {
            if !model.isForeground || model.isCaptured {
                Colors.WowPay.surface.ignoresSafeArea().overlay { Text(Localized.SecretPhrase.ContentHidden.description).padding() }
                    .accessibilityIdentifier("offlineIdentityPrivacyCover")
            }
        }
    }

    private var exportContentType: UTType {
        switch model.pendingExport?.kind {
        case .publicKey: .plainText
        case .backup, .proof, nil: .json
        }
    }

    private func page(_ route: OfflineIdentityRoute?) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                Text(Localized.phaseoneOfflineDescription).font(.wowPay.caption).wowPayCard()
                if let error = model.errorMessage {
                    Text(error).foregroundStyle(Colors.WowPay.multisigErrorText).font(.wowPay.caption).wowPayCard()
                }
                if model.isBusy {
                    ProgressView().frame(maxWidth: .infinity)
                }
                content(route)
            }
            .padding(Spacing.wowPay.page)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .wowPayBackground()
        .navigationTitle(title(route))
        .navigationBarTitleDisplayMode(.inline)
        .scrollDismissesKeyboard(.interactively)
        .disabled(model.isBusy)
    }

    @ViewBuilder
    private func content(_ route: OfflineIdentityRoute?) -> some View {
        @Bindable var model = model
        switch route {
        case nil:
            if let state = model.publicState {
                LabeledContent(Localized.phaseoneOfflineFingerprint, value: state.fingerprint).font(.wowPay.caption).wowPayCard()
                Text(state.backupSaved ? Localized.phaseoneOfflineBackupSaved : Localized.phaseoneOfflineBackupRequired).font(.wowPay.body)
                NavigationLink(Localized.phaseoneOfflineBackup, value: OfflineIdentityRoute.backup).buttonStyle(.wowPay())
                Button(Localized.phaseoneOfflineExportPublicKey, action: model.exportPublicKey).buttonStyle(.wowPaySecondary()).disabled(!state.backupSaved)
                NavigationLink(Localized.phaseoneOfflineImportChallenge, value: OfflineIdentityRoute.challenge).buttonStyle(.wowPaySecondary()).disabled(!state.backupSaved)
            } else {
                NavigationLink(Localized.phaseoneOfflineCreate, value: OfflineIdentityRoute.create).buttonStyle(.wowPay())
                NavigationLink(Localized.phaseoneOfflineRestore, value: OfflineIdentityRoute.restore).buttonStyle(.wowPaySecondary())
            }
        case .create:
            Text(Localized.phaseoneOfflineCreateWarning).font(.wowPay.body).wowPayCard()
            Button(Localized.phaseoneOfflineCreate) { Task { await model.create() } }.buttonStyle(.wowPay())
        case .backup:
            Text(Localized.phaseoneOfflinePasswordHint).font(.wowPay.caption)
            SecureField(Localized.phaseoneOfflinePassword, text: $model.password).textContentType(.newPassword).privacySensitive().wowPayInput()
            SecureField(Localized.phaseoneOfflineConfirmPassword, text: $model.passwordConfirmation).textContentType(.newPassword).privacySensitive().wowPayInput()
            Button(Localized.phaseoneOfflineBackup) { Task { await model.exportBackup() } }.buttonStyle(.wowPay())
        case .restore:
            Button(Localized.phaseoneOfflineImportBackup) { model.isImporting = true }.buttonStyle(.wowPaySecondary())
            if !model.importedBackup.isEmpty {
                Text(Localized.phaseoneOfflineDocumentLoaded).font(.wowPay.caption)
            }
            SecureField(Localized.phaseoneOfflinePassword, text: $model.password).privacySensitive().wowPayInput()
            Button(Localized.phaseoneOfflineRestore) { Task { await model.restore() } }.buttonStyle(.wowPay()).disabled(model.importedBackup.isEmpty)
        case .challenge:
            Button(Localized.phaseoneOfflineImportChallenge) { model.isImporting = true }.buttonStyle(.wowPaySecondary())
            if let challenge = model.challenge {
                VStack(alignment: .leading, spacing: Spacing.space12) {
                    LabeledContent(Localized.phaseoneOfflineAccount, value: challenge.userId)
                    LabeledContent(Localized.phaseoneOfflinePurpose, value: purpose(challenge.purpose))
                    LabeledContent(Localized.phaseoneOfflineLabel, value: challenge.label)
                    LabeledContent(Localized.phaseoneOfflineExpires, value: challenge.expiresAt)
                    Text(Localized.phaseoneOfflineFingerprint).bold()
                    Text(challenge.fingerprint)
                    Text(Localized.phaseoneOfflineActionHash).bold()
                    Text(challenge.actionHash)
                }
                .font(.wowPay.caption).textSelection(.enabled).wowPayCard()
                Text(Localized.phaseoneOfflineChallengeWarning).font(.wowPay.caption).wowPayCard()
                Button(Localized.phaseoneOfflineSign) { Task { await model.signChallenge() } }.buttonStyle(.wowPay())
            }
        case .proof:
            Text(Localized.phaseoneOfflineProofReady).font(.wowPay.body).wowPayCard()
            Button(Localized.phaseoneOfflineExportProof, action: model.exportProof).buttonStyle(.wowPay()).disabled(model.proof == nil)
        }
    }

    private func title(_ route: OfflineIdentityRoute?) -> String {
        switch route {
        case nil: Localized.phaseoneOfflineTitle
        case .create: Localized.phaseoneOfflineCreate
        case .restore: Localized.phaseoneOfflineRestore
        case .backup: Localized.phaseoneOfflineBackup
        case .challenge: Localized.phaseoneOfflineImportChallenge
        case .proof: Localized.phaseoneOfflineExportProof
        }
    }

    private func purpose(_ purpose: BsvHomeOfflineIdentityPurpose) -> String {
        switch purpose {
        case .enroll: Localized.phaseoneOfflinePurposeEnroll
        case .recover: Localized.phaseoneOfflinePurposeRecover
        case .register: Localized.phaseoneOfflinePurposeRegister
        }
    }

    private func updateCapture() {
        let captured = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.contains { $0.traitCollection.sceneCaptureState == .active }
        model.captureChanged(captured)
    }
}
