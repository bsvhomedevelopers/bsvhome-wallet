// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.AlienError
import protocol BsvHomeCore.AlienProvider
import class BsvHomeCore.AlienResponse
import struct BsvHomeCore.AlienTarget
import Primitives

public actor NativeProvider {
    private enum RequestPolicy {
        case standard
        case paymail
        case platform
    }

    private let session: URLSession
    private let baseURL: URL?
    private let requestPolicy: RequestPolicy
    private var rateLimitDates: [String: Date] = [:]

    public init(session: URLSession = .shared) {
        self.session = session
        baseURL = nil
        requestPolicy = .standard
    }

    init(paymailSession: URLSession) {
        session = paymailSession
        baseURL = nil
        requestPolicy = .paymail
    }

    public static func paymail() -> NativeProvider {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpShouldSetCookies = false
        configuration.httpCookieStorage = nil
        configuration.urlCache = nil
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.timeoutIntervalForRequest = 30
        configuration.timeoutIntervalForResource = 30
        return NativeProvider(paymailSession: URLSession(configuration: configuration, delegate: RejectRedirectsDelegate(), delegateQueue: nil))
    }

    public init(baseURL: String) throws {
        try self.init(baseURL: baseURL, policy: .standard)
    }

    public static func platform(baseURL: String) throws -> NativeProvider {
        try NativeProvider(baseURL: baseURL, policy: .platform)
    }

    private init(baseURL: String, policy: RequestPolicy) throws {
        guard let url = URL(string: baseURL), ["http", "https"].contains(url.scheme), url.host != nil,
              url.user == nil, url.password == nil, url.query == nil, url.fragment == nil
        else {
            throw AlienError.RequestError(msg: "invalid request scope")
        }
        self.baseURL = url
        requestPolicy = policy
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpShouldSetCookies = false
        configuration.httpCookieStorage = nil
        configuration.urlCache = nil
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.timeoutIntervalForRequest = policy == .platform ? 40 : 20
        configuration.timeoutIntervalForResource = policy == .platform ? 40 : 30
        session = URLSession(configuration: configuration, delegate: RejectRedirectsDelegate(), delegateQueue: nil)
    }

    init(platformSession: URLSession, baseURL: URL) {
        session = platformSession
        self.baseURL = baseURL
        requestPolicy = .platform
    }

    private func response(for request: URLRequest) async throws -> (Data, URLResponse) {
        guard requestPolicy == .platform else { return try await session.data(for: request) }
        let limit = 4_194_304
        let (bytes, response) = try await session.bytes(for: request)
        defer { bytes.task.cancel() }
        guard response.expectedContentLength <= limit else {
            throw AlienError.ResponseError(msg: "INVALID_RESPONSE")
        }
        var data = Data()
        for try await byte in bytes {
            guard data.count < limit else { throw AlienError.ResponseError(msg: "INVALID_RESPONSE") }
            data.append(byte)
        }
        return (data, response)
    }

    func permits(_ url: URL) -> Bool {
        guard let baseURL else { return true }
        let basePath = baseURL.standardized.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let path = url.standardized.path
        return url.scheme == baseURL.scheme && url.host == baseURL.host
            && (url.port ?? (url.scheme == "https" ? 443 : 80)) == (baseURL.port ?? (baseURL.scheme == "https" ? 443 : 80))
            && url.user == nil && url.password == nil && url.fragment == nil
            && (path == "/" + basePath || path.hasPrefix("/" + basePath + "/"))
    }

    static func retryAfterDate(_ value: String?, now: Date) -> Date {
        guard let value else { return now.addingTimeInterval(1) }
        if let seconds = TimeInterval(value), seconds.isFinite, seconds >= 0 {
            return min(now.addingTimeInterval(max(seconds, 1)), .distantFuture)
        }
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        formatter.dateFormat = "EEE, dd MMM yyyy HH:mm:ss z"
        return min(max(formatter.date(from: value) ?? now, now.addingTimeInterval(1)), .distantFuture)
    }
}

extension NativeProvider: AlienProvider {
    public func request(target: AlienTarget) async throws -> AlienResponse {
        do {
            let request = try target.asRequest()
            guard let url = request.url, permits(url) else {
                throw AlienError.RequestError(msg: "request is outside configured scope")
            }
            if requestPolicy == .paymail, let host = url.host, let date = rateLimitDates[host], date > Date() {
                return AlienResponse(status: 429, data: Data(#"{"code":"RATE_LIMITED","message":"Paymail rate limited"}"#.utf8))
            }
            let (data, response) = try await response(for: request)
            let statusCode = (response as? HTTPURLResponse)?.statusCode

            if requestPolicy == .paymail, let host = url.host, let response = response as? HTTPURLResponse, response.statusCode == 429 {
                rateLimitDates[host] = Self.retryAfterDate(response.value(forHTTPHeaderField: "Retry-After"), now: Date())
            }

            return AlienResponse(status: statusCode.map(UInt16.init), data: data)
        } catch {
            if isNetworkError(error) {
                throw AlienError.Offline
            }
            if (error as NSError).domain == NSURLErrorDomain {
                throw AlienError.ResponseError(msg: error.localizedDescription)
            }
            throw error
        }
    }
}
