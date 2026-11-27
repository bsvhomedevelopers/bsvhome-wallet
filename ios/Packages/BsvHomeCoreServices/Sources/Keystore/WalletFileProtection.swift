import Foundation
import enum BsvHomeCore.BsvHomeServiceError

public enum WalletFileProtection {
    public static func apply(to url: URL) throws {
        let values = try url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
        guard values.isSymbolicLink != true else { throw BsvHomeServiceError.Store(msg: "Wallet storage contains an unsupported file") }
        #if os(iOS)
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: url.path)
        #endif
        var protectedURL = url
        var attributes = URLResourceValues()
        attributes.isExcludedFromBackup = true
        try protectedURL.setResourceValues(attributes)
        guard try protectedURL.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == true else {
            throw BsvHomeServiceError.Store(msg: "Wallet backup exclusion could not be verified")
        }
        #if os(iOS) && !targetEnvironment(simulator)
            let value = try FileManager.default.attributesOfItem(atPath: url.path)[.protectionKey]
            let protection = (value as? FileProtectionType)?.rawValue ?? value as? String
            guard protection == FileProtectionType.complete.rawValue else {
                throw BsvHomeServiceError.Store(msg: "Wallet file protection could not be verified")
            }
        #endif
        if values.isDirectory == true {
            for child in try FileManager.default.contentsOfDirectory(at: url, includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey]) {
                try apply(to: child)
            }
        }
    }
}
