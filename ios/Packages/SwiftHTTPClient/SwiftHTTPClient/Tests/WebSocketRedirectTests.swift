import Foundation
import Testing
@testable import WebSocketClient

struct WebSocketRedirectTests {
    @Test(arguments: [WebSocketRedirectPolicy.follow, .reject])
    func redirectPolicy(policy: WebSocketRedirectPolicy) async throws {
        let source = try #require(URL(string: "https://paymail.bsvhome.com/api/v1/wallet-events"))
        let destination = try #require(URL(string: "https://redirect.example/wallet-events"))
        let response = try #require(HTTPURLResponse(url: source, statusCode: 302, httpVersion: "HTTP/1.1", headerFields: ["Location": destination.absoluteString]))
        let session = URLSession(configuration: .ephemeral)
        defer { session.invalidateAndCancel() }
        let delegate = WebSocketSessionDelegate(didOpen: {}, didClose: { _, _ in }, redirectPolicy: policy)
        let request = await withCheckedContinuation { continuation in
            delegate.urlSession(session, task: session.dataTask(with: source), willPerformHTTPRedirection: response, newRequest: URLRequest(url: destination)) {
                continuation.resume(returning: $0)
            }
        }
        switch policy {
        case .follow: #expect(request?.url == destination)
        case .reject: #expect(request == nil)
        }
    }
}
