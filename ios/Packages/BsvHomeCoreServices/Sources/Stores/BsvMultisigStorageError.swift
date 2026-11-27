enum BsvMultisigStorageError: Error, Equatable {
    case unavailable
    case invalidRevision
    case invalidEntry
}
