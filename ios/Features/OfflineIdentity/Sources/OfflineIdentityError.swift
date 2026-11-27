enum OfflineIdentityError: Error, Equatable {
    case invalidDocument
    case invalidPassword
    case cryptographyUnavailable
    case keyAlreadyExists
    case keyMissing
    case backupRequired
    case staleOperation
    case unavailable
}
