import Foundation
import struct BsvHomeCore.BsvHomeOfflineIdentityChallenge
import class BsvHomeCore.BsvHomeOfflineIdentityService
import Keychain
import Localization
import Observation
import SwiftUI

@MainActor
@Observable
public final class OfflineIdentityViewModel: Identifiable {
    var path: [OfflineIdentityRoute] = []
    var password = ""
    var passwordConfirmation = ""
    var importedBackup = ""
    var challengeJSON = ""
    var challenge: BsvHomeOfflineIdentityChallenge?
    var publicState: OfflineIdentityPublicState?
    var proof: String?
    var document: OfflineIdentityDocument?
    var pendingExport: OfflineIdentityExport?
    var isExporting = false
    var isImporting = false
    var isForeground = false
    var isCaptured = false
    var isBusy = false
    var errorMessage: String?
    private let vault: OfflineIdentityVault
    private let authenticate: @MainActor @Sendable () async throws -> Void
    private var generation: UInt64 = 0

    public init(keychain: any Keychain, rootURL: URL? = nil, authenticate: @escaping @MainActor @Sendable () async throws -> Void) {
        vault = OfflineIdentityVault(keychain: keychain, rootURL: rootURL)
        self.authenticate = authenticate
    }

    func load() async {
        await run { generation in
            let state = try await self.vault.publicState()
            try self.requireCurrent(generation)
            self.publicState = state
        }
    }

    func create() async {
        await run { generation in
            let state = try await self.vault.create()
            self.publicState = state
            try self.requireCurrent(generation)
            self.path = [.backup]
        }
    }

    func exportBackup() async {
        guard password == passwordConfirmation else { errorMessage = Localized.phaseoneOfflinePasswordMismatch; return }
        let password = password
        self.password = ""
        passwordConfirmation = ""
        await run { generation in
            let value = try await self.vault.backup(password: password)
            try self.requireCurrent(generation)
            guard let state = self.publicState else { throw OfflineIdentityError.keyMissing }
            self.export(OfflineIdentityExport(kind: .backup(fingerprint: state.fingerprint), text: value, filename: "wowpay-offline-identity-backup.json", generation: self.generation))
        }
    }

    func restore() async {
        let password = password
        let document = importedBackup
        self.password = ""
        passwordConfirmation = ""
        await run { generation in
            let state = try await self.vault.restore(document: document, password: password)
            self.publicState = state
            try self.requireCurrent(generation)
            self.importedBackup = ""
            self.path = []
        }
    }

    func exportPublicKey() {
        guard isForeground, !isCaptured, let state = publicState, state.backupSaved else { errorMessage = Localized.phaseoneOfflineBackupRequired; return }
        export(OfflineIdentityExport(kind: .publicKey, text: state.publicKey, filename: "wowpay-offline-identity-public-key.txt", generation: generation))
    }

    func reviewChallenge() {
        do {
            guard isForeground, !isCaptured, let state = publicState, state.backupSaved else { throw OfflineIdentityError.backupRequired }
            challenge = try BsvHomeOfflineIdentityService().reviewChallenge(challengeJson: challengeJSON, publicKey: state.publicKey)
            errorMessage = nil
        } catch {
            challenge = nil
            errorMessage = Localized.phaseoneOfflineInvalidDocument
        }
    }

    func signChallenge() async {
        let frozen = challengeJSON
        guard let challenge, let state = publicState else { return }
        await run { generation in
            guard frozen == self.challengeJSON, challenge.fingerprint == state.fingerprint else { throw OfflineIdentityError.staleOperation }
            let proof = try await self.vault.proof(challengeJSON: frozen)
            try self.requireCurrent(generation)
            self.proof = proof
            self.path.append(.proof)
        }
    }

    func exportProof() {
        guard isForeground, !isCaptured, let proof else { return }
        export(OfflineIdentityExport(kind: .proof, text: proof, filename: "wowpay-offline-identity-proof.json", generation: generation))
    }

    func imported(_ result: Result<URL, Error>) {
        do {
            guard isForeground, !isCaptured else { throw OfflineIdentityError.unavailable }
            let url = try result.get()
            let access = url.startAccessingSecurityScopedResource()
            defer {
                if access {
                    url.stopAccessingSecurityScopedResource()
                }
            }
            let file = try FileHandle(forReadingFrom: url)
            defer { try? file.close() }
            guard let bytes = try file.read(upToCount: 16385), bytes.count <= 16384, let text = String(data: bytes, encoding: .utf8) else { throw OfflineIdentityError.invalidDocument }
            if path.last == .restore {
                _ = try BsvHomeOfflineIdentityService().readBackup(document: text)
                importedBackup = text
            } else {
                challengeJSON = text
                reviewChallenge()
            }
        } catch {
            errorMessage = Localized.phaseoneOfflineInvalidDocument
        }
    }

    func exported(_ result: Result<URL, Error>) async {
        guard let pending = pendingExport else { return }
        pendingExport = nil
        document = nil
        guard pending.generation == generation, isForeground, !isCaptured else { return }
        do {
            _ = try result.get()
            switch pending.kind {
            case let .backup(fingerprint):
                publicState = try await vault.markBackupSaved(fingerprint: fingerprint)
                guard pending.generation == generation, isForeground, !isCaptured else { return }
                password = ""
                passwordConfirmation = ""
                path = []
            case .publicKey, .proof:
                break
            }
        } catch {
            errorMessage = Localized.phaseoneOfflineSaveCancelled
        }
    }

    func sceneChanged(_ phase: ScenePhase) {
        isForeground = phase == .active
        if phase == .background {
            clearSensitiveState()
        }
    }

    func captureChanged(_ captured: Bool) {
        isCaptured = captured
        if captured {
            clearSensitiveState()
        }
    }

    func clearSensitiveState() {
        generation &+= 1
        password = ""
        passwordConfirmation = ""
        importedBackup = ""
        challengeJSON = ""
        challenge = nil
        proof = nil
        pendingExport = nil
        document = nil
        isExporting = false
        isImporting = false
    }

    private func export(_ pending: OfflineIdentityExport) {
        pendingExport = pending
        document = OfflineIdentityDocument(text: pending.text)
        isExporting = true
    }

    func navigationChanged(_ previous: [OfflineIdentityRoute], _ current: [OfflineIdentityRoute]) {
        if current.count < previous.count {
            clearSensitiveState()
        }
    }

    private func requireCurrent(_ generation: UInt64) throws {
        guard self.generation == generation, isForeground, !isCaptured else { throw OfflineIdentityError.staleOperation }
    }

    private func run(_ action: @escaping @MainActor (UInt64) async throws -> Void) async {
        guard !isBusy, isForeground, !isCaptured else { return }
        let generation = generation
        let originalPath = path
        isBusy = true
        errorMessage = nil
        defer { isBusy = false }
        do {
            try await authenticate()
            try Task.checkCancellation()
            try requireCurrent(generation)
            guard originalPath == path else { throw OfflineIdentityError.staleOperation }
            try await action(generation)
            guard self.generation == generation, isForeground, !isCaptured else { clearSensitiveState(); return }
        } catch {
            if self.generation == generation {
                errorMessage = Localized.phaseoneOfflineOperationFailed
            }
        }
    }
}
