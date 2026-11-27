import Foundation
import enum BsvHomeCore.AlienError
import protocol BsvHomeCore.AlienProvider
import class BsvHomeCore.AlienResponse
import struct BsvHomeCore.AlienTarget
import protocol BsvHomeCore.BsvHomeMpcDeviceProof
import enum BsvHomeCore.BsvHomeMpcError
import enum BsvHomeCore.BsvHomePlatformAccountError
import class BsvHomeCore.BsvHomePlatformAccountService
import struct BsvHomeCore.BsvHomePlatformAccountSnapshot
import class BsvHomeCore.BsvHomePlatformAlternativeService
import struct BsvHomeCore.BsvHomePlatformAlternativeSnapshot
import protocol BsvHomeCore.BsvHomePlatformAuthenticator
import struct BsvHomeCore.BsvHomePlatformHdOverview
import class BsvHomeCore.BsvHomePlatformHdService
import enum BsvHomeCore.BsvHomePlatformPasskeyOperation
import struct BsvHomeCore.BsvHomePlatformSession
import protocol BsvHomeCore.BsvHomeSecureStore
@testable import PhaseOne
import Testing

@MainActor
struct PlatformAccountStateTests {
    @Test
    func expiredSessionClearsDisplayedAccountAndPreservesWalletStorage() async throws {
        let provider = AccountFixtureProvider()
        let store = AccountFixtureStore()
        try store.set(key: "retained-wallet", value: "synthetic-retained-material")
        let service = try BsvHomePlatformAccountService(provider: provider, store: store, authenticator: AccountFixtureAuthenticator(), baseUrl: "https://account.example/v2/api/", language: "en", deviceId: "test-installation")
        let challenge = try await service.emailStart(email: "test@example.com")
        _ = try await service.emailFinish(challengeId: challenge.id, code: "001234")
        let model = PlatformAccountViewModel(service: service)
        await model.load()
        #expect(model.profile?.email == "test@example.com")
        #expect(model.session?.userId == "42")
        provider.expire()
        await model.load()
        #expect(model.profile == nil)
        #expect(model.session == nil)
        #expect(model.passkeys.isEmpty)
        #expect(model.devices.isEmpty)
        #expect(try store.get(key: "retained-wallet") == "synthetic-retained-material")
    }

    @Test
    func hdUnauthorizedRefreshesAccountProjectionDuringAnExistingAccountLoad() async throws {
        let provider = AccountFixtureProvider()
        let store = AccountFixtureStore()
        try store.set(key: "retained-wallet", value: "synthetic-retained-material")
        let service = try BsvHomePlatformAccountService(provider: provider, store: store, authenticator: AccountFixtureAuthenticator(), baseUrl: "https://account.example/v2/api/", language: "en", deviceId: "test-installation")
        let challenge = try await service.emailStart(email: "test@example.com")
        _ = try await service.emailFinish(challengeId: challenge.id, code: "001234")
        let snapshot = try #require(await service.snapshot())
        let gate = AccountSnapshotGate()
        let deferred = AccountDeferredService(noHandle: .init())
        deferred.service = service
        deferred.firstSnapshot = snapshot
        deferred.gate = gate
        let model = PlatformAccountViewModel(service: deferred)
        await model.load()
        #expect(model.session?.userId == "42")
        let pending = Task { await model.load() }
        await gate.waitUntilRequested()
        #expect(model.isBusy)
        provider.expire()
        var callbacks = 0
        let hdService = AccountHdFixture(noHandle: .init())
        hdService.service = service
        let hd = PlatformHdViewModel(service: hdService, onSessionInvalidated: {
            callbacks += 1
            #expect(hdService.cancellations == 1)
            await model.refreshSessionProjection()
        })
        hd.label = "old account"
        hd.consent = true
        await hd.load()
        #expect(callbacks == 1)
        #expect(hd.label.isEmpty)
        #expect(!hd.consent)
        #expect(model.session == nil)
        await gate.finish(snapshot)
        await pending.value
        #expect(model.session == nil)
        #expect(model.profile == nil)
        #expect(try service.session() == nil)
        #expect(try store.get(key: "retained-wallet") == "synthetic-retained-material")
    }

    @Test
    func alternativeUnauthorizedCannotClearAReplacementSessionWhileUpdatingTheAccountProjection() async throws {
        let provider = AccountFixtureProvider()
        let store = AccountFixtureStore()
        let service = try BsvHomePlatformAccountService(provider: provider, store: store, authenticator: AccountFixtureAuthenticator(), baseUrl: "https://account.example/v2/api/", language: "en", deviceId: "test-installation")
        let challenge = try await service.emailStart(email: "test@example.com")
        _ = try await service.emailFinish(challengeId: challenge.id, code: "001234")
        let model = PlatformAccountViewModel(service: service)
        await model.load()
        var callbacks = 0
        let alternativeService = AccountAlternativeFixture(noHandle: .init())
        alternativeService.service = BsvHomePlatformAlternativeService(account: service, device: AccountFixtureDevice())
        let alternative = PlatformAlternativeViewModel(service: alternativeService, onSessionInvalidated: {
            callbacks += 1
            #expect(alternativeService.cancellations == 1)
            provider.resume(userId: "43")
            do {
                let challenge = try await service.emailStart(email: "new@example.com")
                _ = try await service.emailFinish(challengeId: challenge.id, code: "001234")
            } catch { Issue.record(error) }
            await model.refreshSessionProjection()
        })
        alternative.publicKey = "old public key"
        alternative.requestId = "old request"
        provider.expire()
        await alternative.load()
        #expect(callbacks == 1)
        #expect(alternative.publicKey.isEmpty)
        #expect(alternative.requestId.isEmpty)
        #expect(model.session?.userId == "43")
        #expect(model.profile?.userId == "43")
        #expect(try service.session()?.userId == "43")
    }
}

private final class AccountFixtureProvider: AlienProvider, @unchecked Sendable {
    private let lock = NSLock()
    private var expired = false
    private var userId = "42"

    func expire() {
        lock.withLock { expired = true }
    }

    func resume(userId: String) {
        lock.withLock { expired = false; self.userId = userId }
    }

    func request(target: AlienTarget) async throws -> AlienResponse {
        if lock.withLock({ expired }) {
            return AlienResponse(status: 401, data: Data(#"{"code":"LOGIN_REQUIRED"}"#.utf8))
        }
        let data: String
        let userId = lock.withLock { self.userId }
        switch URL(string: target.url)?.path {
        case "/v2/api/portal/identity/email/start": data = #"{"challengeId":"11111111111111111111111111111111","expiresAt":"2099-01-01T00:00:00Z","retryAfterSeconds":60}"#
        case "/v2/api/portal/identity/email/finish": data = #"{"token":"\#(String(repeating: userId == "42" ? "A" : "B", count: 43))","userId":"\#(userId)","expiresAt":"2099-01-01T00:00:00Z","authMethod":"EMAIL_OTP","walletAuthority":false}"#
        case "/v2/api/portal/account/profile": data = #"{"userId":"\#(userId)","email":"test@example.com","nickname":"Test"}"#
        case "/v2/api/portal/identity/passkey/list": data = "[]"
        case "/v2/api/portal/account/devices/page": data = #"{"rows":[],"total":0}"#
        case "/v2/api/portal/identity/alternative/keys": data = "[]"
        default: throw AlienError.RequestError(msg: "unexpected fixture request")
        }
        return AlienResponse(status: 200, data: Data((#"{"success":true,"code":"OK","data":"# + data + "}").utf8))
    }
}

private final class AccountFixtureStore: BsvHomeSecureStore, @unchecked Sendable {
    private let lock = NSLock()
    private var values: [String: String] = [:]
    func get(key: String) throws -> String? {
        lock.withLock { values[key] }
    }

    func set(key: String, value: String) throws {
        lock.withLock { values[key] = value }
    }

    func remove(key: String) throws {
        _ = lock.withLock { values.removeValue(forKey: key) }
    }
}

private final class AccountFixtureAuthenticator: BsvHomePlatformAuthenticator {
    func authenticate(operation _: BsvHomePlatformPasskeyOperation, optionsJson _: String) async throws -> String {
        throw BsvHomePlatformAccountError.Cancelled
    }

    func verifyOfflineProof(publicKey _: String, actionHash _: String, signature _: String) throws -> Bool {
        false
    }
}

private final class AccountFixtureDevice: BsvHomeMpcDeviceProof {
    func publicKey() async throws -> String {
        throw BsvHomeMpcError.NativeUnavailable
    }

    func signMessage(message _: String) async throws -> String {
        throw BsvHomeMpcError.NativeUnavailable
    }
}

private final class AccountHdFixture: BsvHomePlatformHdService, @unchecked Sendable {
    var service: BsvHomePlatformAccountService?
    private let lock = NSLock()
    private var cancellationCount = 0
    var cancellations: Int { lock.withLock { cancellationCount } }

    override func overview() async throws -> BsvHomePlatformHdOverview {
        guard let service else { throw BsvHomePlatformAccountError.InvalidConfiguration }
        _ = try await service.profile()
        return BsvHomePlatformHdOverview(localWallets: [], accounts: [])
    }

    override func cancelWorkflow() throws {
        guard let service else { throw BsvHomePlatformAccountError.InvalidConfiguration }
        try service.cancel()
        lock.withLock { cancellationCount += 1 }
    }
}

private final class AccountAlternativeFixture: BsvHomePlatformAlternativeService, @unchecked Sendable {
    var service: BsvHomePlatformAlternativeService?
    private let lock = NSLock()
    private var cancellationCount = 0
    var cancellations: Int { lock.withLock { cancellationCount } }

    override func snapshot() async throws -> BsvHomePlatformAlternativeSnapshot {
        guard let service else { throw BsvHomePlatformAccountError.InvalidConfiguration }
        return try await service.snapshot()
    }

    override func cancelWorkflow() throws {
        guard let service else { throw BsvHomePlatformAccountError.InvalidConfiguration }
        try service.cancelWorkflow()
        lock.withLock { cancellationCount += 1 }
    }
}

private final class AccountDeferredService: BsvHomePlatformAccountService, @unchecked Sendable {
    var service: BsvHomePlatformAccountService?
    var firstSnapshot: BsvHomePlatformAccountSnapshot?
    var gate: AccountSnapshotGate?
    private let lock = NSLock()
    private var calls = 0

    override func session() throws -> BsvHomePlatformSession? {
        guard let service else { throw BsvHomePlatformAccountError.InvalidConfiguration }
        return try service.session()
    }

    override func snapshot() async throws -> BsvHomePlatformAccountSnapshot? {
        guard let service, let firstSnapshot, let gate else { throw BsvHomePlatformAccountError.InvalidConfiguration }
        let call = lock.withLock {
            calls += 1
            return calls
        }
        if call == 1 {
            return firstSnapshot
        }
        if call == 2 {
            return await gate.response()
        }
        return try await service.snapshot()
    }
}

private actor AccountSnapshotGate {
    private var requested = false
    private var waiting: CheckedContinuation<Void, Never>?
    private var reply: CheckedContinuation<BsvHomePlatformAccountSnapshot?, Never>?

    func waitUntilRequested() async {
        if requested {
            return
        }
        await withCheckedContinuation { waiting = $0 }
    }

    func response() async -> BsvHomePlatformAccountSnapshot? {
        requested = true
        waiting?.resume()
        waiting = nil
        return await withCheckedContinuation { reply = $0 }
    }

    func finish(_ snapshot: BsvHomePlatformAccountSnapshot) {
        reply?.resume(returning: snapshot)
        reply = nil
    }
}
