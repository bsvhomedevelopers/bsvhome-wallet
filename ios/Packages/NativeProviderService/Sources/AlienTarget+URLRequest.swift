// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.AlienError
import func BsvHomeCore.alienMethodToString
import struct BsvHomeCore.AlienTarget

extension AlienTarget {
    func asRequest() throws -> URLRequest {
        guard let url = URL(string: url) else {
            let error = AlienError.RequestError(msg: "invalid url: \(url)")
            throw error
        }
        var request = URLRequest(url: url)
        request.httpMethod = alienMethodToString(method: method)
        request.allHTTPHeaderFields = headers
        if let body {
            request.httpBody = body
        }
        return request
    }
}
