enum BsvColdWalletStorageError: Error {
    case unavailable
    case invalidRevision
    case oversized
}
