import Foundation
import Keychain
import LocalAuthentication

final class OfflineIdentityKeychainMock: Keychain, @unchecked Sendable {
    private let lock = NSLock()
    private var values: [String: Data] = [:]
    private(set) var access: Accessibility?
    private(set) var synchronizedAccess = false

    func accessibility(_ accessibility: Accessibility, authenticationPolicy _: AuthenticationPolicy) -> any Keychain {
        lock.withLock { access = accessibility }
        return self
    }

    func authenticationContext(_: LAContext) -> any Keychain {
        self
    }

    func get(_ key: String, ignoringAttributeSynchronizable: Bool) throws -> String? {
        try getString(key, ignoringAttributeSynchronizable: ignoringAttributeSynchronizable)
    }

    func getString(_ key: String, ignoringAttributeSynchronizable: Bool) throws -> String? {
        try getData(key, ignoringAttributeSynchronizable: ignoringAttributeSynchronizable).flatMap { String(data: $0, encoding: .utf8) }
    }

    func getData(_ key: String, ignoringAttributeSynchronizable: Bool) throws -> Data? {
        lock.withLock {
            synchronizedAccess = synchronizedAccess || ignoringAttributeSynchronizable
            return values[key]
        }
    }

    func set(_ value: String, key: String, ignoringAttributeSynchronizable: Bool) throws {
        try set(Data(value.utf8), key: key, ignoringAttributeSynchronizable: ignoringAttributeSynchronizable)
    }

    func set(_ value: Data, key: String, ignoringAttributeSynchronizable: Bool) throws {
        lock.withLock {
            synchronizedAccess = synchronizedAccess || ignoringAttributeSynchronizable
            values[key] = value
        }
    }

    func remove(_ key: String, ignoringAttributeSynchronizable _: Bool) throws {
        lock.withLock { _ = values.removeValue(forKey: key) }
    }

    func removeAll() {
        lock.withLock { values.removeAll() }
    }
}
