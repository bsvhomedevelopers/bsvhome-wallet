import Foundation
import enum BsvHomeCore.BsvHomePlatformAccountError
import protocol BsvHomeCore.BsvHomePlatformAccountServiceProtocol
import struct BsvHomeCore.BsvHomePlatformClosure
import struct BsvHomeCore.BsvHomePlatformDevice
import struct BsvHomeCore.BsvHomePlatformEmailChallenge
import struct BsvHomeCore.BsvHomePlatformPasskey
import struct BsvHomeCore.BsvHomePlatformProfile
import struct BsvHomeCore.BsvHomePlatformSession
import Localization
import Observation

@Observable
@MainActor
public final class PlatformAccountViewModel {
    private let service: any BsvHomePlatformAccountServiceProtocol
    public private(set) var session: BsvHomePlatformSession?
    var profile: BsvHomePlatformProfile?
    var passkeys: [BsvHomePlatformPasskey] = []
    var devices: [BsvHomePlatformDevice] = []
    var canRevokePasskey = false
    var challenge: BsvHomePlatformEmailChallenge?
    var closure: BsvHomePlatformClosure?
    var email = ""
    var code = ""
    var nickname = ""
    var label = ""
    var reason = ""
    var acknowledgeAddress = false
    var acknowledgeRetention = false
    private(set) var isBusy = false
    private(set) var error: String?
    private var revision: UInt = 0
    private var projectionRevision: UInt = 0
    private var projectionRequestRevision: UInt = 0
    private var refreshAfterOperation = false

    public init(service: any BsvHomePlatformAccountServiceProtocol) {
        self.service = service
    }

    public func load() async {
        guard let snapshot = await value({ try await $0.snapshot() }) else { return }
        if session?.userId != snapshot?.session.userId {
            clearAccount()
        }
        applySession(snapshot?.session)
        profile = snapshot?.profile
        passkeys = snapshot?.passkeys ?? []
        devices = snapshot?.devices ?? []
        canRevokePasskey = snapshot?.canRevokePasskey ?? false
        nickname = profile?.nickname ?? ""
    }

    func requestCode() async {
        let email = email
        if let challenge = await value({ try await $0.emailStart(email: email) }) {
            self.challenge = challenge; code = ""
        }
    }

    func loginEmail(reopen: Bool = false) async {
        guard let challenge else { return }
        let code = code
        let result = await value { service in
            if reopen {
                return try await service.reopenFinish(challengeId: challenge.id, code: code)
            }
            return try await service.emailFinish(challengeId: challenge.id, code: code)
        }
        if let result {
            applySession(result); self.challenge = nil; self.code = ""; await load()
        }
    }

    func loginPasskey() async {
        if let result = await value({ try await $0.passkeyLogin() }) {
            applySession(result); challenge = nil; code = ""; await load()
        }
    }

    func addPasskey() async {
        let label = label
        if await value({ try await $0.registerPasskey(label: label) }) != nil {
            await load()
        }
    }

    func revokePasskey(_ id: String) async {
        if await value({ try await $0.revokePasskey(id: id) }) != nil {
            await load()
        }
    }

    func revokeDevice(_ id: String) async {
        let reason = reason
        if await value({ try await $0.revokeDevice(id: id, reason: reason) }) != nil {
            await load()
        }
    }

    func saveProfile() async {
        let nickname = nickname
        let language = profile?.language ?? (Bundle.main.preferredLocalizations.first?.hasPrefix("zh") == true ? "zh-CN" : "en")
        let timezone = profile?.timezone ?? TimeZone.current.identifier
        if let profile = await value({ try await $0.updateProfile(nickname: nickname, language: language, timezone: timezone) }) {
            self.profile = profile
        }
    }

    func renew() async {
        if let session = await value({ try await $0.renewSession() }) {
            applySession(session)
        }
    }

    func logout() async {
        _ = await value { try await $0.logout() }
        clearInput()
        await load()
    }

    func closureStatus() async {
        if let closure = await value({ try await $0.closureStatus() }) {
            self.closure = closure
        }
    }

    func requestClosure() async {
        let address = acknowledgeAddress
        let retention = acknowledgeRetention
        if let closure = await value({ try await $0.closureRequest(acknowledgeAddress: address, acknowledgeRetention: retention) }) {
            self.closure = closure
            acknowledgeAddress = false
            acknowledgeRetention = false
        }
    }

    func finishClosure() async {
        guard let closure else { return }
        let address = acknowledgeAddress
        let retention = acknowledgeRetention
        if let result = await value({ try await $0.closureFinish(id: closure.id, acknowledgeAddress: address, acknowledgeRetention: retention) }) {
            self.closure = result
            clearInput()
            await load()
        }
    }

    func cancelClosure() async {
        guard let closure else { return }
        if let result = await value({ try await $0.closureCancel(id: closure.id) }) {
            self.closure = result
        }
    }

    func reopen() async {
        if let challenge = await value({ try await $0.reopenStart() }) {
            self.challenge = challenge; code = ""
        }
    }

    func changeEmailStart() async {
        let email = email
        if let challenge = await value({ try await $0.changeEmailStart(email: email) }) {
            self.challenge = challenge; code = ""
        }
    }

    func changeEmailFinish() async {
        guard let challenge else { return }
        let code = code
        if let profile = await value({ try await $0.changeEmailFinish(id: challenge.id, code: code) }) {
            self.profile = profile
            self.challenge = nil
            self.code = ""
        }
    }

    private func clearAccount() {
        applySession(nil)
        profile = nil
        passkeys = []
        devices = []
        canRevokePasskey = false
        closure = nil
        label = ""
        reason = ""
        nickname = ""
        clearInput()
    }

    public func refreshSessionProjection() async {
        if isBusy {
            refreshAfterOperation = true
        }
        let current = projectionRevision
        projectionRequestRevision &+= 1
        let request = projectionRequestRevision
        let service = service
        do {
            let session = try await Task.detached(priority: .userInitiated) { try service.session() }.value
            guard current == projectionRevision, request == projectionRequestRevision else { return }
            let ownerChanged = self.session?.userId != session?.userId
            revision &+= 1
            if ownerChanged {
                clearAccount()
            }
            applySession(session)
            if ownerChanged, session != nil, !isBusy {
                await load()
            }
        } catch {
            guard current == projectionRevision, request == projectionRequestRevision else { return }
            self.error = (error as? BsvHomePlatformAccountError)?.phaseOneMessage ?? Localized.phaseoneServiceError
        }
    }

    private func applySession(_ session: BsvHomePlatformSession?) {
        self.session = session
        projectionRevision &+= 1
    }

    private func refreshWhenAvailable() async {
        if isBusy {
            refreshAfterOperation = true
            return
        }
        await load()
    }

    public func clearInput() {
        challenge = nil
        email = ""
        code = ""
        acknowledgeAddress = false
        acknowledgeRetention = false
    }

    public func cancel() {
        revision &+= 1
        clearInput()
        do { try service.cancel() }
        catch { self.error = (error as? BsvHomePlatformAccountError)?.phaseOneMessage ?? Localized.phaseoneServiceError }
    }

    func value<T: Sendable>(_ call: @escaping @Sendable (any BsvHomePlatformAccountServiceProtocol) async throws -> T) async -> T? {
        guard !isBusy else { return nil }
        isBusy = true
        error = nil
        let current = revision
        let service = service
        defer {
            isBusy = false
            if refreshAfterOperation {
                refreshAfterOperation = false
                Task { await refreshWhenAvailable() }
            }
        }
        do {
            let result = try await Task.detached(priority: .userInitiated) { try await call(service) }.value
            guard current == revision else { return nil }
            return result
        } catch {
            guard current == revision else { return nil }
            if let error = error as? BsvHomePlatformAccountError, error == .LoginRequired || error == .SessionChanged {
                clearAccount()
                if error == .SessionChanged {
                    refreshAfterOperation = true
                }
            }
            self.error = (error as? BsvHomePlatformAccountError)?.phaseOneMessage ?? Localized.phaseoneServiceError
            return nil
        }
    }
}
