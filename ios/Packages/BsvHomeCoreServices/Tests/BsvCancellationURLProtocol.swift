import Foundation

final class BsvCancellationURLProtocol: URLProtocol, @unchecked Sendable {
    static let state = State()
    private let requestLock = NSLock()
    private var suspended = false

    override class func canInit(with _: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        if Self.state.beginRequest() {
            requestLock.withLock { suspended = true }
            return
        }
        guard let url = request.url, let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"]) else {
            client?.urlProtocol(self, didFailWithError: URLError(.badURL))
            return
        }
        do {
            let body = try historyResponse()
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: body)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {
        if requestLock.withLock({ suspended }) {
            Self.state.stopRequest()
        }
    }

    private func historyResponse() throws -> Data {
        guard request.url?.path.hasSuffix("/addresses/history/all") == true else {
            throw URLError(.unsupportedURL)
        }
        let body: Data
        if let data = request.httpBody {
            body = data
        } else if let stream = request.httpBodyStream {
            stream.open()
            defer { stream.close() }
            var data = Data()
            var buffer = [UInt8](repeating: 0, count: 1024)
            while true {
                let count = stream.read(&buffer, maxLength: buffer.count)
                guard count >= 0 else { throw stream.streamError ?? URLError(.cannotDecodeContentData) }
                if count == 0 {
                    break
                }
                data.append(contentsOf: buffer.prefix(count))
            }
            body = data
        } else {
            throw URLError(.cannotDecodeContentData)
        }
        guard let object = try JSONSerialization.jsonObject(with: body) as? [String: Any],
              let addresses = object["addresses"] as? [String], addresses.count == 1,
              let address = addresses.first
        else {
            throw URLError(.cannotDecodeContentData)
        }
        let section: [String: Any] = ["result": [], "error": ""]
        return try JSONSerialization.data(withJSONObject: [["address": address, "confirmed": section, "unconfirmed": section]])
    }

    final class State: @unchecked Sendable {
        private let lock = NSLock()
        private var suspendAfter: Int = .max
        private var requests = 0
        private var stopped = 0

        var requestCount: Int { lock.withLock { requests } }
        var stoppedCount: Int { lock.withLock { stopped } }

        func reset(suspendAfter: Int) {
            lock.withLock {
                self.suspendAfter = suspendAfter
                requests = 0
                stopped = 0
            }
        }

        func allowResponses() {
            lock.withLock { suspendAfter = .max }
        }

        func beginRequest() -> Bool {
            lock.withLock {
                requests += 1
                return requests > suspendAfter
            }
        }

        func stopRequest() {
            lock.withLock { stopped += 1 }
        }
    }
}
