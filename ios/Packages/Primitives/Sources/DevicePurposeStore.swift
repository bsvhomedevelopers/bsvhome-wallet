import Foundation

public enum DevicePurpose: String, Sendable {
    case daily
    case coldWallet
}

public struct DevicePurposeStore: Sendable {
    private enum StorageError: Error {
        case unavailable
        case invalidRecord
    }

    private let recordURL: URL?

    public static var appGroup: Self {
        let directory = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: Constants.appGroupIdentifier)
        return Self(recordURL: directory?.appending(path: "wowpay-device-purpose", directoryHint: .notDirectory))
    }

    init(recordURL: URL?) {
        self.recordURL = recordURL
    }

    public var permitsNetworkAccess: Bool {
        (try? read()) == .daily
    }

    public func read() throws -> DevicePurpose? {
        guard let recordURL else { throw StorageError.unavailable }
        let data: Data
        do {
            data = try Data(contentsOf: recordURL)
        } catch let error as CocoaError where error.code == .fileReadNoSuchFile {
            return nil
        }
        guard let purpose = DevicePurpose(rawValue: String(decoding: data, as: UTF8.self)) else {
            throw StorageError.invalidRecord
        }
        return purpose
    }

    @MainActor
    public func confirm(_ purpose: DevicePurpose) throws -> DevicePurpose {
        guard let recordURL else { throw StorageError.unavailable }
        if let existing = try read() {
            return existing
        }
        #if os(iOS)
            let options: Data.WritingOptions = [.atomic, .completeFileProtectionUntilFirstUserAuthentication]
        #else
            let options: Data.WritingOptions = .atomic
        #endif
        try Data(purpose.rawValue.utf8).write(to: recordURL, options: options)
        return purpose
    }
}
