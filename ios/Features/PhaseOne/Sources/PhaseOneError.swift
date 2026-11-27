import enum BsvHomeCore.BsvHomeMpcError
import enum BsvHomeCore.BsvHomePlatformAccountError
import Localization

extension BsvHomePlatformAccountError {
    var phaseOneMessage: String {
        switch self {
        case .Cancelled: Localized.phaseoneCancelled
        case .AuthenticationUnavailable: Localized.phaseonePasskeyUnavailable
        case let .Api(code): Localized.phaseoneServiceError + " (" + code + ")"
        case .LoginRequired, .SessionChanged: Localized.phaseoneLoginRequired
        default: Localized.phaseoneServiceError
        }
    }
}

extension BsvHomeMpcError {
    var phaseOneMessage: String {
        switch self {
        case let .Platform(error): error.phaseOneMessage
        case .Cancelled: Localized.phaseoneCancelled
        case .BackupMismatch: Localized.phaseoneBackupMismatch
        case .OperationPending: Localized.phaseoneOperationPending
        case .NativeUnavailable, .SecureStorageUnavailable: Localized.phaseoneStorageUnavailable
        default: Localized.phaseoneServiceError
        }
    }
}
