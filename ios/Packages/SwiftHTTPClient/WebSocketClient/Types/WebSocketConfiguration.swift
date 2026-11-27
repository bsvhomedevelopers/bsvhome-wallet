// Copyright (c). Gem Wallet. All rights reserved.

import Foundation

public struct WebSocketConfiguration: Sendable {
    public let requestProvider: any WebSocketRequestProvider
    public let reconnection: any Reconnectable
    public let sessionConfiguration: URLSessionConfiguration
    public let redirectPolicy: WebSocketRedirectPolicy
    public let receiveBufferLimit: Int?
    public let maximumMessageSize: Int?
    public let queueMessagesWhileConnecting: Bool

    public init(
        requestProvider: any WebSocketRequestProvider,
        reconnection: any Reconnectable,
        sessionConfiguration: URLSessionConfiguration = .default,
        redirectPolicy: WebSocketRedirectPolicy = .follow,
        receiveBufferLimit: Int? = nil,
        maximumMessageSize: Int? = nil,
        queueMessagesWhileConnecting: Bool = true,
    ) {
        self.requestProvider = requestProvider
        self.reconnection = reconnection
        self.sessionConfiguration = sessionConfiguration
        self.redirectPolicy = redirectPolicy
        self.receiveBufferLimit = receiveBufferLimit.map { max(1, $0) }
        self.maximumMessageSize = maximumMessageSize.map { max(1, $0) }
        self.queueMessagesWhileConnecting = queueMessagesWhileConnecting
    }

    public init(
        request: URLRequest,
        reconnection: any Reconnectable,
        sessionConfiguration: URLSessionConfiguration = .default,
        redirectPolicy: WebSocketRedirectPolicy = .follow,
        receiveBufferLimit: Int? = nil,
        maximumMessageSize: Int? = nil,
        queueMessagesWhileConnecting: Bool = true,
    ) {
        self.init(
            requestProvider: StaticRequestProvider(request: request),
            reconnection: reconnection,
            sessionConfiguration: sessionConfiguration,
            redirectPolicy: redirectPolicy,
            receiveBufferLimit: receiveBufferLimit,
            maximumMessageSize: maximumMessageSize,
            queueMessagesWhileConnecting: queueMessagesWhileConnecting,
        )
    }

    public init(
        url: URL,
        reconnection: any Reconnectable,
        sessionConfiguration: URLSessionConfiguration = .default,
        redirectPolicy: WebSocketRedirectPolicy = .follow,
        receiveBufferLimit: Int? = nil,
        maximumMessageSize: Int? = nil,
        queueMessagesWhileConnecting: Bool = true,
    ) {
        self.init(
            requestProvider: StaticRequestProvider(url: url),
            reconnection: reconnection,
            sessionConfiguration: sessionConfiguration,
            redirectPolicy: redirectPolicy,
            receiveBufferLimit: receiveBufferLimit,
            maximumMessageSize: maximumMessageSize,
            queueMessagesWhileConnecting: queueMessagesWhileConnecting,
        )
    }
}
