import Foundation
@testable import NativeProviderService
import Testing

struct PortalHttpsLiveTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["PORTAL_HTTPS_LIVE_TEST"] == "1"), arguments: [
        "/auth/paymail/config",
        "/auth/multisig/wallets",
    ])
    func configuredPortalHttpsPassesAts(path: String) async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpShouldSetCookies = false
        configuration.timeoutIntervalForRequest = 20
        let session = URLSession(configuration: configuration, delegate: RejectRedirectsDelegate(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let url = try #require(URL(string: "https://paymail.bsvhome.com/api/portal" + path))
        var request = URLRequest(url: url)
        request.setValue("zh-CN", forHTTPHeaderField: "language")
        let (data, response) = try await session.data(for: request)
        let httpResponse = try #require(response as? HTTPURLResponse)
        try #require(httpResponse.statusCode == 401)
        let envelope = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        #expect(envelope["code"] as? String == "LOGIN_REQUIRED")
        #expect(envelope["message"] is String)
        #expect(envelope["success"] == nil)
    }
}
