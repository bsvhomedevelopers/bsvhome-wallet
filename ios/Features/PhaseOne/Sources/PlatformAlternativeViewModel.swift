import Foundation
import enum BsvHomeCore.BsvHomePlatformAccountError
import struct BsvHomeCore.BsvHomePlatformAlternativeChallenge
import struct BsvHomeCore.BsvHomePlatformAlternativeKey
import struct BsvHomeCore.BsvHomePlatformAlternativeRequest
import protocol BsvHomeCore.BsvHomePlatformAlternativeServiceProtocol
import Localization
import Observation

@Observable
@MainActor
public final class PlatformAlternativeViewModel {
    private let service: any BsvHomePlatformAlternativeServiceProtocol
    private let onSessionInvalidated: @MainActor @Sendable () async -> Void
    private var revision: UInt = 0
    private var proofJSON: String?
    private var lastError: BsvHomePlatformAccountError?
    private(set) var keys: [BsvHomePlatformAlternativeKey] = []
    private(set) var request: BsvHomePlatformAlternativeRequest?
    private(set) var challenge: BsvHomePlatformAlternativeChallenge?
    private(set) var isBusy = false
    private(set) var error: String?
    private(set) var proofReady = false
    private(set) var fingerprint: String?
    var publicKey = ""
    var label = ""
    var requestId = ""
    var exportDocument: PhaseOneDocument?

    public init(service: any BsvHomePlatformAlternativeServiceProtocol, onSessionInvalidated: @escaping @MainActor @Sendable () async -> Void = {}) {
        self.service = service
        self.onSessionInvalidated = onSessionInvalidated
    }

    func load() async {
        guard let snapshot = await value({ try await $0.snapshot() }) else { return }
        keys = snapshot.keys
        request = snapshot.request
        if let request {
            requestId = request.id
        }
    }

    func checkPublicKey() async {
        fingerprint = nil
        let publicKey = publicKey.trimmingCharacters(in: .whitespacesAndNewlines)
        if let fingerprint = await value({ try $0.publicKeyFingerprint(publicKey: publicKey) }) {
            self.publicKey = publicKey
            self.fingerprint = fingerprint
        }
    }

    func enrollment() async {
        let publicKey = publicKey.trimmingCharacters(in: .whitespacesAndNewlines)
        let label = label
        clearProof()
        if let challenge = await value({ try await $0.enrollment(publicKey: publicKey, label: label) }) {
            present(challenge)
        }
    }

    func recovery(_ id: String) async {
        let publicKey = publicKey.trimmingCharacters(in: .whitespacesAndNewlines)
        clearProof()
        if let challenge = await value({ try await $0.recovery(keyId: id, publicKey: publicKey) }) {
            present(challenge)
        }
    }

    func revoke(_ id: String) async {
        if await value({ try await $0.revoke(id: id) }) != nil {
            await load()
        }
    }

    func exportChallenge() async {
        guard let challenge else { return }
        if let text = await value({ try $0.challengeJson(id: challenge.id) }) {
            exportDocument = PhaseOneDocument(data: Data(text.utf8))
        }
    }

    func importPublicKey(_ url: URL) async {
        guard let text = await readFile(url, maximumBytes: 1024) else { return }
        let publicKey = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if let fingerprint = await value({ try $0.publicKeyFingerprint(publicKey: publicKey) }) {
            self.publicKey = publicKey
            self.fingerprint = fingerprint
        }
    }

    func importProof(_ url: URL) async {
        guard let challenge else { return }
        clearProof()
        guard let text = await readFile(url, maximumBytes: 16384) else { return }
        if await value({ try $0.reviewProof(id: challenge.id, proofJson: text) }) != nil {
            guard self.challenge?.id == challenge.id, self.challenge?.actionHash == challenge.actionHash else { return }
            proofJSON = text
            proofReady = true
        }
    }

    func submitProof() async {
        guard let challenge, let proofJSON else { return }
        switch challenge.purpose {
        case "Enroll":
            if await value({ try await $0.enroll(id: challenge.id, proofJson: proofJSON) }) != nil {
                closeChallenge()
                await load()
            } else if outcomeUnknown {
                closeChallenge()
                error = Localized.phaseoneOperationPending
            }
        case "Recover":
            if let request = await value({ try await $0.request(id: challenge.id, proofJson: proofJSON) }) {
                self.request = request
                requestId = request.id
                closeChallenge()
            } else if outcomeUnknown {
                requestId = challenge.id
                closeChallenge()
                error = Localized.phaseoneOperationPending
            }
        case "Register":
            if let request = await value({ try await $0.register(id: challenge.id, proofJson: proofJSON) }) {
                self.request = request
                requestId = request.id
                closeChallenge()
                await load()
            } else if outcomeUnknown {
                requestId = challenge.id
                closeChallenge()
                error = Localized.phaseoneOperationPending
            }
        default: error = Localized.phaseoneOfflineInvalidDocument
        }
    }

    func refreshRequest() async {
        let id = requestId
        if let request = await value({ try await $0.status(id: id) }) {
            self.request = request
        }
    }

    func cancelRequest() async {
        guard let request else { return }
        if let request = await value({ try await $0.cancel(id: request.id) }) {
            self.request = request
        }
    }

    func registration() async {
        guard let request else { return }
        clearProof()
        if let challenge = await value({ try await $0.registration(id: request.id) }) {
            present(challenge)
        }
    }

    func closeChallenge() {
        revision &+= 1
        challenge = nil
        exportDocument = nil
        clearProof()
    }

    func fileFailed() {
        error = Localized.phaseoneOfflineInvalidDocument
    }

    public func cancel() {
        revision &+= 1
        closeChallenge()
        if isBusy {
            do { try service.cancelWorkflow() }
            catch { self.error = (error as? BsvHomePlatformAccountError)?.phaseOneMessage ?? Localized.phaseoneServiceError }
        }
    }

    public func clearMemory() {
        error = nil
        lastError = nil
        cancel()
        keys = []
        request = nil
        requestId = ""
        publicKey = ""
        fingerprint = nil
        label = ""
    }

    private func clearProof() {
        proofJSON = nil
        proofReady = false
    }

    private var outcomeUnknown: Bool {
        switch lastError {
        case .NetworkUnavailable, .InvalidResponse: true
        default: false
        }
    }

    private func present(_ challenge: BsvHomePlatformAlternativeChallenge) {
        revision &+= 1
        clearProof()
        exportDocument = nil
        self.challenge = challenge
    }

    private func readFile(_ url: URL, maximumBytes: Int) async -> String? {
        let current = revision
        do {
            let text = try await Task.detached(priority: .userInitiated) {
                let access = url.startAccessingSecurityScopedResource()
                defer {
                    if access {
                        url.stopAccessingSecurityScopedResource()
                    }
                }
                let file = try FileHandle(forReadingFrom: url)
                defer { try? file.close() }
                var data = Data()
                while let part = try file.read(upToCount: min(8192, maximumBytes + 1 - data.count)), !part.isEmpty {
                    data.append(part)
                    guard data.count <= maximumBytes else { throw BsvHomePlatformAccountError.InvalidInput }
                }
                guard let text = String(data: data, encoding: .utf8) else { throw BsvHomePlatformAccountError.InvalidInput }
                return text
            }.value
            guard current == revision else { return nil }
            return text
        } catch {
            guard current == revision else { return nil }
            self.error = Localized.phaseoneOfflineInvalidDocument
            return nil
        }
    }

    private func value<T: Sendable>(_ call: @escaping @Sendable (any BsvHomePlatformAlternativeServiceProtocol) async throws -> T) async -> T? {
        guard !isBusy else { return nil }
        isBusy = true
        error = nil
        lastError = nil
        let current = revision
        let service = service
        defer { isBusy = false }
        do {
            let result = try await Task.detached(priority: .userInitiated) { try await call(service) }.value
            guard current == revision else { return nil }
            return result
        } catch {
            if let accountError = error as? BsvHomePlatformAccountError, accountError == .LoginRequired || accountError == .SessionChanged {
                if current == revision {
                    clearMemory()
                }
                await onSessionInvalidated()
                return nil
            }
            guard current == revision else { return nil }
            lastError = error as? BsvHomePlatformAccountError
            self.error = (error as? BsvHomePlatformAccountError)?.phaseOneMessage ?? Localized.phaseoneServiceError
            return nil
        }
    }
}
