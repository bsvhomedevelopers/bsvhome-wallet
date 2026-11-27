// Copyright (c). Gem Wallet. All rights reserved.

import Foundation

final class WebSocketSessionDelegate: NSObject, URLSessionWebSocketDelegate, Sendable {
    private let didOpen: @Sendable () -> Void
    private let didClose: @Sendable (URLSessionWebSocketTask.CloseCode, Data?) -> Void
    private let redirectPolicy: WebSocketRedirectPolicy

    init(
        didOpen: @escaping @Sendable () -> Void,
        didClose: @escaping @Sendable (URLSessionWebSocketTask.CloseCode, Data?) -> Void,
        redirectPolicy: WebSocketRedirectPolicy = .follow,
    ) {
        self.didOpen = didOpen
        self.didClose = didClose
        self.redirectPolicy = redirectPolicy
    }

    func urlSession(
        _: URLSession,
        task _: URLSessionTask,
        willPerformHTTPRedirection _: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping @Sendable (URLRequest?) -> Void,
    ) {
        switch redirectPolicy {
        case .follow: completionHandler(request)
        case .reject: completionHandler(nil)
        }
    }

    func urlSession(
        _: URLSession,
        webSocketTask _: URLSessionWebSocketTask,
        didOpenWithProtocol _: String?,
    ) {
        didOpen()
    }

    func urlSession(
        _: URLSession,
        webSocketTask _: URLSessionWebSocketTask,
        didCloseWith closeCode: URLSessionWebSocketTask.CloseCode,
        reason: Data?,
    ) {
        didClose(closeCode, reason)
    }
}
