// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import class BsvHomeCore.BsvHomeDeviceKeyService
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Primitives
import WebSocketClient

public struct AuthenticatedRequestProvider: WebSocketRequestProvider {
    private let deviceKeyService: BsvHomeDeviceKeyService

    public init(deviceKeyService: BsvHomeDeviceKeyService) {
        self.deviceKeyService = deviceKeyService
    }

    public func makeRequest() throws -> URLRequest {
        let stream = try deviceKeyService.deviceStreamRequest()
        guard let url = URL(string: stream.url) else {
            throw AnyError("invalid device stream url: \(stream.url)")
        }
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.setValue(stream.authorization, forHTTPHeaderField: "Authorization")
        return request
    }
}
