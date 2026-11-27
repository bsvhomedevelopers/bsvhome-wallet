import Foundation
import struct BsvHomeCore.AlienTarget
@testable import NativeProviderService
import Testing

struct NativeProviderScopeTests {
    @Test
    func paymailCooldownRejectsAnotherRequestWithoutTransportOrDelayedReplay() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [PaymailRateLimitURLProtocol.self]
        let provider = NativeProvider(paymailSession: URLSession(configuration: configuration))
        let target = AlienTarget(url: "https://rate-limit.example/paymail", method: .get, headers: nil, body: nil)
        _ = try await provider.request(target: target)
        _ = try await provider.request(target: target)
        #expect(PaymailRateLimitURLProtocol.requests.count == 1)
    }

    @Test
    func paymailRespectsRetryAfterSecondsAndHTTPDate() {
        let now = Date(timeIntervalSince1970: 1_445_412_480)
        #expect(NativeProvider.retryAfterDate("30", now: now) == now.addingTimeInterval(30))
        #expect(NativeProvider.retryAfterDate("Wed, 21 Oct 2015 07:29:00 GMT", now: now) == now.addingTimeInterval(60))
        #expect(NativeProvider.retryAfterDate(nil, now: now) == now.addingTimeInterval(1))
        #expect(NativeProvider.retryAfterDate("invalid", now: now) == now.addingTimeInterval(1))
        #expect(NativeProvider.retryAfterDate("-1", now: now) == now.addingTimeInterval(1))
        #expect(NativeProvider.retryAfterDate("1e308", now: now) == .distantFuture)
    }

    @Test(arguments: [
        ("https://api-dev.hj0252.com/api/portal/wallet-auth/challenge", true),
        ("https://api-dev.hj0252.com:443/api/portal/auth/swaps/pairs", true),
        ("https://api-dev.hj0252.com:7001/api/portal/auth/swaps/pairs", false),
        ("https://example.com/api/portal/auth/swaps/pairs", false),
        ("http://api-dev.hj0252.com/api/portal/auth/swaps/pairs", false),
        ("http://47.84.135.53/api/portal/auth/swaps/pairs", false),
        ("https://47.84.135.53/api/portal/auth/swaps/pairs", false),
        ("https://api-dev.hj0252.com/api/admin/auth/swaps/pairs", false),
        ("https://api-dev.hj0252.com/api/portal-other/auth/swaps/pairs", false),
        ("https://api-dev.hj0252.com/api/portal/../admin/auth/swaps/pairs", false),
        ("https://user@api-dev.hj0252.com/api/portal/auth/swaps/pairs", false),
    ])
    func scopeRestrictsOriginAndPortalPath(url: String, expected: Bool) async throws {
        let provider = try NativeProvider(baseURL: "https://api-dev.hj0252.com/api/portal")
        #expect(try await provider.permits(#require(URL(string: url))) == expected)
    }

    @Test(arguments: [
        ("https://api-dev.hj0252.com/v2/api/portal/wallet-auth/challenge", true),
        ("https://api-dev.hj0252.com/v2/api/portal/auth/multisig/wallets", true),
        ("https://api-dev.hj0252.com/v2/api/portal/auth/swaps/pairs", true),
        ("https://api-dev.hj0252.com/api/portal/auth/multisig/wallets", false),
        ("https://api-dev.hj0252.com/v2/api/portal-other/auth/multisig/wallets", false),
        ("https://api-dev.hj0252.com/v2/api/portal/../admin/wallets", false),
        ("https://elsewhere.example/v2/api/portal/auth/multisig/wallets", false),
        ("http://api-dev.hj0252.com/v2/api/portal/auth/multisig/wallets", false),
    ])
    func v2ScopeRejectsLegacyAndUnrelatedEndpoints(url: String, expected: Bool) async throws {
        let provider = try NativeProvider.platform(baseURL: "https://api-dev.hj0252.com/v2/api/portal")
        #expect(try await provider.permits(#require(URL(string: url))) == expected)
    }

    @Test
    func redirectsAreRejected() async throws {
        let delegate = RejectRedirectsDelegate()
        let session = URLSession(configuration: .ephemeral)
        let target = try #require(URL(string: "https://api-dev.hj0252.com/api/portal"))
        let task = session.dataTask(with: target)
        let response = try #require(HTTPURLResponse(url: target, statusCode: 302, httpVersion: nil, headerFields: nil))
        let result = await withCheckedContinuation { continuation in
            delegate.urlSession(session, task: task, willPerformHTTPRedirection: response, newRequest: URLRequest(url: target)) {
                continuation.resume(returning: $0)
            }
        }
        #expect(result == nil)
        session.invalidateAndCancel()
    }
}

private final class PaymailRateLimitURLProtocol: URLProtocol, @unchecked Sendable {
    static let requests = RequestCounter()

    override class func canInit(with request: URLRequest) -> Bool {
        request.url?.host == "rate-limit.example"
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let url = request.url,
              let response = HTTPURLResponse(url: url, statusCode: 429, httpVersion: nil, headerFields: ["Retry-After": "2"])
        else {
            client?.urlProtocol(self, didFailWithError: URLError(.badURL))
            return
        }
        Self.requests.increment()
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(#"{"code":"RATE_LIMITED","message":"wait"}"#.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

private final class RequestCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var value = 0

    var count: Int { lock.withLock { value } }
    func increment() {
        lock.withLock { value += 1 }
    }
}
