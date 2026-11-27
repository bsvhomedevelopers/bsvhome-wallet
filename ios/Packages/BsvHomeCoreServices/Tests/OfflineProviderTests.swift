import BsvHomeCore
import BsvHomeCoreServices
import Testing

struct OfflineProviderTests {
    @Test(arguments: ["https://example.invalid/balance", "http://localhost/broadcast", "file:///private/wallet"])
    func everyRequestIsRejected(url: String) async {
        let provider = OfflineProvider()
        await #expect(throws: (any Error).self) {
            try await provider.request(target: AlienTarget(url: url, method: .get, headers: nil, body: nil))
        }
    }
}
