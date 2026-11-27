import Foundation
import enum BsvHomeCore.AlienError
import struct BsvHomeCore.AlienTarget
@testable import NativeProviderService
import Testing

struct PlatformTransportTests {
    @Test(arguments: ["declared", "streamed"])
    func rejectsOversizedResponseBeforeDeliveringItToCore(path: String) async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [PlatformResponseURLProtocol.self]
        let baseURL = try #require(URL(string: "https://platform.example/v2/api/"))
        let provider = NativeProvider(platformSession: URLSession(configuration: configuration), baseURL: baseURL)
        await #expect(throws: AlienError.self) {
            try await provider.request(target: AlienTarget(url: baseURL.absoluteString + "portal/" + path, method: .post, headers: nil, body: Data("{}".utf8)))
        }
    }

    @Test(arguments: [
        ("https://platform.example/v2/api/portal/identity/email/start", true),
        ("https://platform.example/api/portal/identity/email/start", false),
        ("https://platform.example/v2/admin/users", false),
        ("https://platform.example/v2/api-other/portal/users", false),
        ("https://elsewhere.example/v2/api/portal/users", false),
    ])
    func limitsV2RequestsToTheirConfiguredEnvironment(url: String, permitted: Bool) async throws {
        let provider = try NativeProvider.platform(baseURL: "https://platform.example/v2/api/")
        #expect(try await provider.permits(#require(URL(string: url))) == permitted)
    }

    @Test(arguments: [
        ("https://paymail.bsvhome.com/api/portal", "https://paymail.bsvhome.com/api/portal/auth/paymail/config", true),
        ("https://paymail.bsvhome.com/api/portal", "https://paymail.bsvhome.com:443/api/portal/auth/multisig/wallets", true),
        ("https://paymail.bsvhome.com/api/portal", "https://api-dev.hj0252.com/v2/api/portal/auth/paymail/config", false),
        ("https://paymail.bsvhome.com/api/portal", "https://paymail.bsvhome.com/v2/api/portal/auth/paymail/config", false),
        ("https://paymail.bsvhome.com/api/portal", "https://paymail.bsvhome.com/api/admin/users", false),
        ("https://paymail.bsvhome.com/api/portal", "https://paymail.bsvhome.com/api/portal-other/auth/paymail/config", false),
        ("https://paymail.bsvhome.com/api/portal", "https://paymail.bsvhome.com/api/portal/../admin/users", false),
        ("https://paymail.bsvhome.com/api/portal", "http://paymail.bsvhome.com/api/portal/auth/paymail/config", false),
        ("https://paymail.bsvhome.com/api/portal", "https://paymail.bsvhome.com:7001/api/portal/auth/paymail/config", false),
        ("https://paymail.bsvhome.com/api/portal", "https://user@paymail.bsvhome.com/api/portal/auth/paymail/config", false),
        ("https://paymail.bsvhome.com/api/portal", "https://paymail.bsvhome.com/api/portal/auth/paymail/config#fragment", false),
        ("https://api-dev.hj0252.com/v2/api/", "https://api-dev.hj0252.com/v2/api/portal/identity/email/start", true),
        ("https://api-dev.hj0252.com/v2/api/", "https://paymail.bsvhome.com/api/portal/auth/paymail/config", false),
        ("https://api-dev.hj0252.com/v2/api/", "https://api-dev.hj0252.com/api/portal/identity/email/start", false),
    ])
    func portalEnvironmentsRejectRequestsOutsideTheirTransportScope(base: String, target: String, permitted: Bool) async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [PlatformResponseURLProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        let baseURL = try #require(URL(string: base))
        let provider = NativeProvider(platformSession: session, baseURL: baseURL)
        let request = AlienTarget(url: target, method: .get, headers: nil, body: nil)

        if permitted {
            _ = try await provider.request(target: request)
        } else {
            await #expect(throws: AlienError.RequestError(msg: "request is outside configured scope")) {
                try await provider.request(target: request)
            }
        }
    }
}

private final class PlatformResponseURLProtocol: URLProtocol, @unchecked Sendable {
    override class func canInit(with _: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let url = request.url, let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: url.lastPathComponent == "declared" ? ["Content-Length": "4194305"] : [:]) else { return }
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        let data = ["declared", "streamed"].contains(url.lastPathComponent) ? Data(repeating: 65, count: 4_194_305) : Data("{}".utf8)
        client?.urlProtocol(self, didLoad: data)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}
