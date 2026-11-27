import struct BsvHomeCore.BsvHomeMpcTransfer

actor MpcTransferGate {
    private var reply: CheckedContinuation<BsvHomeMpcTransfer, Never>?
    private var ready: CheckedContinuation<Void, Never>?

    func response() async -> BsvHomeMpcTransfer {
        await withCheckedContinuation {
            reply = $0
            ready?.resume()
            ready = nil
        }
    }

    func waitUntilRequested() async {
        if reply != nil {
            return
        }
        await withCheckedContinuation { ready = $0 }
    }

    func finish(_ transfer: BsvHomeMpcTransfer) {
        reply?.resume(returning: transfer)
        reply = nil
    }
}
