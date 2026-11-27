actor BsvMultisigAsyncGate {
    private var continuation: CheckedContinuation<Void, Never>?
    private var startedContinuation: CheckedContinuation<Void, Never>?
    private var hasStarted = false

    func wait() async {
        await withCheckedContinuation {
            continuation = $0
            hasStarted = true
            startedContinuation?.resume()
            startedContinuation = nil
        }
    }

    func started() async {
        if hasStarted {
            return
        }
        await withCheckedContinuation { startedContinuation = $0 }
    }

    func release() {
        continuation?.resume()
        continuation = nil
    }
}
