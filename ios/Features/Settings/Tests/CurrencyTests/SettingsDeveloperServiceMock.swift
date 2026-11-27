import Foundation
import BsvHomeCore

final class SettingsDeveloperServiceMock: BsvHomeDeveloperService, @unchecked Sendable {
    private let lock = NSLock()
    private var recordedCalls: [String] = []
    var calls: [String] { lock.withLock { recordedCalls } }
    private func record(_ call: String) {
        lock.withLock { recordedCalls.append(call) }
    }

    override func deviceId() async throws -> String {
        record("device"); return "public-device-fixture"
    }

    override func pushToken() async throws -> String {
        record("push"); return "public-push-fixture"
    }

    override func platformStore() async throws -> BsvHomeCore.PlatformStore {
        record("store"); return .local
    }

    override func clearPendingTransactions() async throws {
        record("clearPending")
    }
}
