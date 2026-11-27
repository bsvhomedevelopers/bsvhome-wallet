import Foundation

public struct BsvHdImportConfiguration: Equatable, Sendable {
    public var pathPreset: BsvHdPathPreset = .default
    public var customPath = ""
    public var legacyNonCompliant = false
    public var expectedAddress = ""

    public init() {}

    public var accountPath: String? {
        switch pathPreset {
        case .default: nil
        case .wowPay: "m/44'/236'/0'"
        case .compatible: "m/44'/145'/0'"
        case .custom: customPath.trimmingCharacters(in: .whitespacesAndNewlines)
        }
    }

    public var isComplete: Bool {
        pathPreset != .custom || !(accountPath ?? "").isEmpty
    }

    public var originalAddress: String? {
        let address = expectedAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        return address.isEmpty ? nil : address
    }

    public func hasOverrides(passphrase: String) -> Bool {
        pathPreset != .default || legacyNonCompliant || originalAddress != nil || !passphrase.isEmpty
    }
}
