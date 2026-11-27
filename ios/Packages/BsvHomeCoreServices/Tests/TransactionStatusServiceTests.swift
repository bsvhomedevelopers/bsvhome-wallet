import Foundation
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import Primitives
import Testing

@Suite(.timeLimit(.minutes(1)))
struct TransactionStatusServiceTests {
    @Test
    func capturesTrackingGenerationBeforeLaunchingCallbackTask() async throws {
        let launch = NSRecursiveLock()
        let generation = Locked(wrappedValue: UInt64(0))
        let calls = AsyncStream<(String, UInt64)>.makeStream()
        let core = BsvHomeTransactionStateServiceMock(
            trackingGeneration: {
                launch.withLock {
                    let captured = generation.wrappedValue
                    generation.wrappedValue += 1
                    return captured
                }
            },
            onTrackWithGeneration: { walletId, _, generation in
                calls.continuation.yield((walletId, generation))
            },
        )
        let status = BsvHomeCoreTransactionStatusService(service: core)

        try launch.withLock {
            status.track(walletId: "receipt-wallet", transactions: [])
            try #require(generation.wrappedValue == 1)
        }
        var iterator = calls.stream.makeAsyncIterator()
        let call = try #require(await iterator.next())
        #expect(call.0 == "receipt-wallet")
        #expect(call.1 == 0)
        calls.continuation.finish()
    }
}
