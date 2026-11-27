// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import NativeProviderService
import Primitives
import PrimitivesTestKit

public extension GatewayService {
    static func mock(session: URLSession? = nil) -> GatewayService {
        GatewayService(
            provider: NativeProvider(session: session ?? .offline),
            preferences: BsvHomePreferencesStoreMock(),
            securePreferences: BsvHomeSecureStoreMock(),
        )
    }
}

private extension URLSession {
    static let offline: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [OfflineURLProtocol.self]
        return URLSession(configuration: configuration)
    }()
}

private final class OfflineURLProtocol: URLProtocol {
    override class func canInit(with _: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        client?.urlProtocol(self, didFailWithError: URLError(.notConnectedToInternet))
    }

    override func stopLoading() {}
}
