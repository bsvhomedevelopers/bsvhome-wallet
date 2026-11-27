actor BsvColdWalletAsyncReply<Value: Sendable> {
    private var continuation: CheckedContinuation<Value, any Error>?
    private var entryWaiters: [CheckedContinuation<Void, Never>] = []
    var started: Bool { continuation != nil }

    func wait() async throws -> Value {
        try await withCheckedThrowingContinuation {
            continuation = $0
            let waiters = entryWaiters
            entryWaiters = []
            waiters.forEach { $0.resume() }
        }
    }

    func waitForEntry() async {
        if started {
            return
        }
        await withCheckedContinuation { entryWaiters.append($0) }
    }

    func complete(_ value: Value) {
        continuation?.resume(returning: value)
        continuation = nil
    }
}
